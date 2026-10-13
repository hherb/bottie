<script lang="ts">
  /** Image-only settings, route disclosure, and optional local model management. */
  import QwenResearchLicenseDisclosure from "$lib/QwenResearchLicenseDisclosure.svelte";
  import {
    localImageAcquisitionPresentation,
    localImageAvailabilityPresentation,
    localImageModelLabel,
    localImageWorkerImportPresentation,
    type LocalImageAcquisitionStatus,
    type LocalImageAvailabilityMetadata,
  } from "$lib/local-image";
  import {
    IMAGE_ASPECT_CHOICES,
    IMAGE_RESOLUTION_CHOICES,
    imageDimensionsAdjustable,
    imageGenerationRequestOptions,
    defaultImageResolution,
    type ImageGenerationExecution,
    type ImageGenerationSize,
    type ImageGenerationResolution,
  } from "$lib/image-options";
  import type { Attachment } from "$lib/presentation";

  type Props = {
    attachments: Attachment[];
    isGenerating: boolean;
    imageExecution: ImageGenerationExecution;
    imageSize: ImageGenerationSize;
    imageResolution?: ImageGenerationResolution;
    imageCount: number;
    generatedImageSourceCount: number;
    imageFeedback: string;
    localImageAvailability: LocalImageAvailabilityMetadata | null;
    localImageAvailabilityFailed: boolean;
    localImageAcquisitionStatus: LocalImageAcquisitionStatus | null;
    localImageAcquisitionFeedback: string;
    localImageWorkerImporting: boolean;
    localImageWorkerImportFeedback: string;
    onimageexecution: (execution: ImageGenerationExecution) => void;
    onimagesize: (size: ImageGenerationSize) => void;
    onimageresolution?: (resolution: ImageGenerationResolution) => void;
    onimagecount: (count: number) => void;
    oninstalllocalimage: () => void;
    oncancellocalimage: () => void;
    onimportlocalimageworker: () => void;
  };
  let {
    attachments,
    isGenerating,
    imageExecution,
    imageSize,
    imageResolution,
    imageCount,
    generatedImageSourceCount,
    imageFeedback,
    localImageAvailability,
    localImageAvailabilityFailed,
    localImageAcquisitionStatus,
    localImageAcquisitionFeedback,
    localImageWorkerImporting,
    localImageWorkerImportFeedback,
    onimageexecution,
    onimagesize,
    onimageresolution = () => {},
    onimagecount,
    oninstalllocalimage,
    oncancellocalimage,
    onimportlocalimageworker,
  }: Props = $props();
  let localImage = $derived(localImageAvailabilityPresentation(localImageAvailability, localImageAvailabilityFailed));
  let localImageReady = $derived(localImageAvailability?.availability === "ready");
  let localModelLabel = $derived(localImageModelLabel(localImageAvailability));
  let readyImageSourceCount = $derived(
    attachments.filter((a) => a.kind === "image" && a.normalization.state === "ready").length +
      generatedImageSourceCount,
  );
  let selectedImageSourceCount = $derived(attachments.length + generatedImageSourceCount);
  let acquisition = $derived(
    localImageAcquisitionStatus ? localImageAcquisitionPresentation(localImageAcquisitionStatus) : null,
  );
  let workerImport = $derived(localImageWorkerImportPresentation(localImageAvailability, localImageWorkerImporting));
  let adjustable = $derived(imageDimensionsAdjustable(imageExecution, localImageAvailability?.modelId));
  let resolution = $derived(imageResolution ?? defaultImageResolution(imageExecution));
  let output = $derived(
    imageGenerationRequestOptions(imageExecution, imageSize, imageCount, resolution, localImageAvailability?.modelId),
  );
</script>

<div class="image-generation-options" aria-label="Image generation options">
  <label>
    <span>Execution</span>
    <select
      aria-label="Execution"
      value={imageExecution}
      disabled={isGenerating}
      onchange={(event) => onimageexecution(event.currentTarget.value as ImageGenerationExecution)}
    >
      <option value="cloud">Cloud</option>
      <option value="local" disabled={!localImageReady}
        >{localModelLabel}{localImageReady ? "" : " · unavailable"}</option
      >
    </select>
  </label>
  <label>
    <span>Aspect ratio</span>
    <select
      aria-label="Aspect ratio"
      value={adjustable ? imageSize : "square"}
      disabled={isGenerating || !adjustable}
      onchange={(event) => onimagesize(event.currentTarget.value as ImageGenerationSize)}
    >
      {#each IMAGE_ASPECT_CHOICES as choice}<option value={choice.value}>{choice.label}</option>{/each}
    </select>
  </label>
  <label>
    <span>Image size</span>
    <select
      aria-label="Image size"
      value={adjustable ? resolution : "small"}
      disabled={isGenerating || !adjustable}
      onchange={(event) => onimageresolution(event.currentTarget.value as ImageGenerationResolution)}
    >
      {#each IMAGE_RESOLUTION_CHOICES as choice}
        {@const dimensions = imageGenerationRequestOptions(
          imageExecution,
          imageSize,
          1,
          choice,
          localImageAvailability?.modelId,
        )}
        <option value={choice}
          >{choice[0].toUpperCase() + choice.slice(1)} · {dimensions.width}×{dimensions.height}</option
        >
      {/each}
    </select>
  </label>
  {#if imageExecution === "cloud"}
    <label>
      <span>Images</span>
      <select
        value={imageCount}
        disabled={isGenerating}
        onchange={(event) => onimagecount(Number(event.currentTarget.value))}
      >
        {#each [1, 2, 3, 4, 5, 6] as count}<option value={count}>{count}</option>{/each}
      </select>
    </label>
    <span class="image-execution"><strong>Cloud</strong> · <code>qwen-image-2.0-2026-03-03</code></span>
    {#if selectedImageSourceCount > 0}
      <span class="image-execution">
        Editing with {readyImageSourceCount} ready source image{readyImageSourceCount === 1
          ? ""
          : "s"}{readyImageSourceCount === selectedImageSourceCount ? "" : ` of ${selectedImageSourceCount} selected`}
      </span>
    {/if}
  {:else}
    <span class="image-execution"><strong>Local</strong> · {output.width}×{output.height} · one image</span>
  {/if}
</div>
{#if imageExecution === "cloud"}
  <p class="image-delivery-note">
    {#if selectedImageSourceCount > 0}
      Your image prompt and normalized source image bytes are sent to Alibaba Model Studio. This may incur provider
      charges. Generated PNG bytes are stored privately on this device.
    {:else}
      Your image prompt is sent to Alibaba Model Studio and may incur provider charges. Generated PNG bytes are stored
      privately on this device.
    {/if}
  </p>
{:else}
  <p class="image-delivery-note">
    Your prompt and generated bytes stay on this device.
    {#if adjustable}Larger images take longer and use more memory.{:else}This local worker supports 512×512 only.{/if}
  </p>
{/if}
<details
  class="local-image-availability"
  aria-label="Local image availability"
  open={Boolean(workerImport.active || acquisition?.active || (imageExecution === "local" && !localImageReady))}
>
  <summary class="local-image-heading">
    <strong>{localModelLabel} details</strong>
    <span class:ready={localImage.state === "ready"}>{localImage.label}</span>
  </summary>
  <div class="local-image-details">
    <div class="local-image-heading">
      <strong>{localModelLabel}</strong>
      <span class:ready={localImage.state === "ready"}>{localImage.label}</span>
    </div>
    {#if localImageAvailability}
      <code>{localImageAvailability.modelId}</code>
      {#if localImageAvailability.runtimeId === "mlx-serve@external"}
        <span>Installed model · Qwen Research License · image server on 127.0.0.1:11234</span>
      {:else}
        <span>
          <code>{localImageAvailability.packageId}</code> · local runtime
          <code>{localImageAvailability.runtimeId}</code>
        </span>
        <span>{localImageAvailability.license} · revision <code>{localImageAvailability.sourceRevision}</code></span>
      {/if}
    {/if}
    <small>{localImage.detail}. No automatic download or cloud fallback.</small>
    {#if workerImport.action !== "none" || workerImport.active}
      <div class="local-image-acquisition" aria-label="Local image worker installation">
        <p>
          Select the reviewed local image worker folder. The selected folder stays inside native code; only exact
          verified bytes are copied into Bottie’s app-owned cache.
        </p>
        <span>{workerImport.detail}</span>
        {#if workerImport.action === "import"}
          <button type="button" disabled={isGenerating} onclick={onimportlocalimageworker}>{workerImport.label}</button>
        {:else}
          <strong>{workerImport.label}</strong>
        {/if}
      </div>
    {/if}
    {#if acquisition && (acquisition.action !== "none" || acquisition.active)}
      <div class="local-image-acquisition" aria-label="Local image model installation">
        <p>
          Download the disclosed immutable package from Hugging Face into Bottie’s app-owned cache. Network access is
          used only for acquisition; generation remains offline.
        </p>
        {#if localImageAcquisitionStatus?.license === "LicenseRef-Qwen-Research"}
          <QwenResearchLicenseDisclosure />
        {/if}
        {#if acquisition.active || acquisition.percent > 0}
          <progress aria-label="Local model download progress" max="100" value={acquisition.percent}
            >{acquisition.percent}%</progress
          >
        {/if}
        <span>{acquisition.detail}</span>
        {#if acquisition.action === "cancel"}
          <strong>{acquisition.label}</strong>
          <button type="button" onclick={oncancellocalimage}>Cancel download</button>
        {:else if ["install", "resume", "retry"].includes(acquisition.action)}
          <button type="button" disabled={isGenerating} onclick={oninstalllocalimage}>{acquisition.label}</button>
        {:else}
          <strong>{acquisition.label}</strong>
        {/if}
      </div>
    {/if}
    {#if localImageAcquisitionFeedback}
      <span class="local-image-acquisition-feedback" role="status">{localImageAcquisitionFeedback}</span>
    {/if}
    {#if localImageWorkerImportFeedback}
      <span class="local-image-acquisition-feedback" role="status">{localImageWorkerImportFeedback}</span>
    {/if}
  </div>
</details>
{#if imageFeedback}<p class="image-generation-feedback" role="status">{imageFeedback}</p>{/if}
