"""Exercise the complete CUDA adapter without claiming a physical CUDA inference result."""

import argparse
import io
import json
from pathlib import Path
from types import SimpleNamespace
import unittest
from unittest.mock import MagicMock, patch

import cuda_worker


class CudaAdapterTests(unittest.TestCase):
    """A fake framework verifies argument binding, GPU placement, and fail-closed token handling."""

    def run_worker(self, memory_gib, tokens=10):
        """Inject only GPU framework boundaries; run the actual request parser and result serializer."""

        class Scheduler:
            """Model a scheduler reconstructed from its own configuration."""

            config = {}

            @classmethod
            def from_config(cls, _config, shift):
                """The adapter must select the official shift-5 schedule."""
                if shift != 5:
                    raise ValueError("unexpected scheduler shift")
                return cls()

        pipe = MagicMock()
        pipe.scheduler = Scheduler()
        pipe.tokenizer.apply_chat_template.return_value = list(range(tokens))
        image = MagicMock()
        image.size = (512, 512)
        image.save.side_effect = lambda output, **_kwargs: output.write(b"\x89PNG\r\n\x1a\n")
        pipe.return_value.images = [image]
        pipe.progress_bar = lambda items: iter(items)

        def infer(*_args, **_kwargs):
            """Model the upstream custom pipeline's progress iterator and image return contract."""
            for _ in pipe.progress_bar(range(50)):
                pass
            return SimpleNamespace(images=[image])

        pipe.side_effect = infer
        factory = MagicMock(return_value=pipe)
        torch = SimpleNamespace(
            cuda=SimpleNamespace(
                is_available=lambda: True,
                is_bf16_supported=lambda: True,
                get_device_properties=lambda _: SimpleNamespace(total_memory=memory_gib * 1024**3),
            ),
            bfloat16="bf16",
            float32="f32",
            Generator=MagicMock(),
        )
        args = argparse.Namespace(
            model_directory="/verified/model", model_id="OpenMOSS-Team/MOSS-Image-SFT", revision="pin"
        )
        data = dict(prompt='{"summary":{"caption":"Literal BOTTIE layout"}}', width=512, height=512, seed=42, steps=50)
        stdin = SimpleNamespace(buffer=io.BytesIO((json.dumps(data) + "\n").encode()))
        output = io.StringIO()
        with patch.dict(
            "sys.modules",
            {"torch": torch, "diffusers": SimpleNamespace(DiffusionPipeline=SimpleNamespace(from_pretrained=factory))},
        ), patch("cuda_worker.sys.addaudithook"), patch("cuda_worker.threading.Thread"), patch(
            "argparse.ArgumentParser.parse_args", return_value=args
        ), patch(
            "cuda_worker.sys.stdin", stdin
        ), patch(
            "cuda_worker.sys.stdout", output
        ), patch.dict(
            "os.environ", {}
        ):
            if tokens > cuda_worker.MAX_TOKENS:
                with self.assertRaises(ValueError):
                    cuda_worker.main()
            else:
                cuda_worker.main()
        return factory, pipe, data, [json.loads(line) for line in output.getvalue().splitlines()]

    def test_complete_result_preserves_layout_seed_model_and_local_only_loading(self):
        """Do not rewrite layout text or silently use remote loading or another image model."""
        factory, pipe, data, events = self.run_worker(48)
        kwargs = factory.call_args.kwargs
        self.assertTrue(kwargs["local_files_only"])
        self.assertEqual(Path(kwargs["custom_pipeline"]).name, "pipeline_moss_image.py")
        self.assertEqual(pipe.call_args.args, (data["prompt"],))
        self.assertEqual(pipe.call_args.kwargs["uncond_skip_layers"], [22])
        self.assertEqual(events[-1]["seed"], 42)
        self.assertEqual(events[-1]["modelId"], "OpenMOSS-Team/MOSS-Image-SFT")
        pipe.to.assert_called_once_with("cuda")

    def test_progress_remains_on_private_stdout_during_framework_output_redirection(self):
        """A caller can observe active denoising and interrupt it rather than guessing model-load time."""
        _, _, _, events = self.run_worker(48)
        steps = [event["step"] for event in events if event["event"] == "progress"]
        self.assertEqual(steps, list(range(1, 51)))

    def test_lower_vram_uses_component_or_sequential_offload(self):
        """Placement policies are tested as policies, not minimum-VRAM hardware qualification."""
        _, component, _, _ = self.run_worker(24)
        component.enable_model_cpu_offload.assert_called_once_with(device="cuda")
        component.enable_sequential_cpu_offload.assert_not_called()
        _, sequential, _, _ = self.run_worker(16)
        sequential.enable_sequential_cpu_offload.assert_called_once_with(device="cuda")
        sequential.to.assert_not_called()

    def test_overlong_tokenized_layout_is_rejected_before_generation(self):
        """Token checks fail rather than relying on the upstream pipeline's silent truncation."""
        _, pipe, _, events = self.run_worker(48, 4097)
        pipe.assert_not_called()
        self.assertEqual([event["event"] for event in events], ["ready"])


if __name__ == "__main__":
    unittest.main()
