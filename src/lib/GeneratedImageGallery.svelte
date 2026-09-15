<script lang="ts">
  import { copyGeneratedAsset } from "$lib/generated-asset-actions";
  import {
    imageEditingLineageLabel,
    imageEditingSourcePresentation,
    type ImageEditingPresentation,
  } from "$lib/image-generation";
  import Icon from "$lib/Icon.svelte";
  import type { GeneratedAsset } from "$lib/presentation";

  type Props = {
    assets: GeneratedAsset[];
    responseId: number;
    responseStored: boolean;
    isGenerating: boolean;
    imageEditing: ImageEditingPresentation;
    onretryimage: (responseId: number) => void;
    onopenasset: (assetId: string) => void;
    onexportasset: (assetId: string) => void;
    ondeleteasset: (assetId: string) => void;
    ontoggleimagesource: (assetId: string) => void;
  };

  let {
    assets,
    responseId,
    responseStored,
    isGenerating,
    imageEditing,
    onretryimage,
    onopenasset,
    onexportasset,
    ondeleteasset,
    ontoggleimagesource,
  }: Props = $props();
  let feedback = $state<{ assetId: string; message: string; failed: boolean } | null>(null);

  /** Copies one already-normalized generated PNG through its opaque preview URL. */
  async function copyGeneratedImage(assetId: string, previewUrl: string): Promise<void> {
    const succeeded = await copyGeneratedAsset(previewUrl);
    feedback = { assetId, message: succeeded ? "Image copied" : "Image copy failed", failed: !succeeded };
  }
</script>

<div class="generated-image-grid" aria-label="Generated images">
  {#each assets as asset (asset.id)}
    <figure class:failed={asset.status === "failed"} class="generated-image">
      {#if asset.previewUrl}
        <img
          src={asset.previewUrl}
          alt={`Generated image ${asset.ordinal + 1}`}
          width={asset.width ?? undefined}
          height={asset.height ?? undefined}
        />
      {:else}
        <div class="generated-image-placeholder">
          <Icon name={asset.status === "failed" ? "x" : "image"} size={24} />
          <span>{asset.status === "pending" ? "Generating…" : asset.status}</span>
        </div>
      {/if}
      {#if asset.status === "completed" && asset.previewUrl}
        {@const imageSourceSelected = imageEditing.selectedSourceIds.includes(asset.id)}
        <div class="generated-image-item-actions">
          <button aria-label={`Open generated image ${asset.ordinal + 1}`} onclick={() => onopenasset(asset.id)}>
            <Icon name="image" size={14} />
          </button>
          <button
            aria-label={`Copy generated image ${asset.ordinal + 1}`}
            onclick={() => void copyGeneratedImage(asset.id, asset.previewUrl!)}
          >
            <Icon name="copy" size={14} />
          </button>
          <button aria-label={`Export generated image ${asset.ordinal + 1}`} onclick={() => onexportasset(asset.id)}>
            <Icon name="file" size={14} />
          </button>
          <button
            class:selected-source={imageSourceSelected}
            aria-label={imageSourceSelected
              ? `Remove generated image ${asset.ordinal + 1} from references`
              : `Use generated image ${asset.ordinal + 1} as a reference`}
            aria-pressed={imageSourceSelected}
            disabled={isGenerating ||
              (!imageSourceSelected &&
                (!imageEditing.active || imageEditing.execution !== "cloud" || !imageEditing.canSelectSource))}
            onclick={() => ontoggleimagesource(asset.id)}
          >
            <Icon name="sparkles" size={14} />
          </button>
          <button
            aria-label={`Delete generated image ${asset.ordinal + 1}`}
            disabled={isGenerating}
            onclick={() => ondeleteasset(asset.id)}
          >
            <Icon name="trash" size={14} />
          </button>
        </div>
        {#if feedback?.assetId === asset.id}
          <span class:error={feedback.failed} class="copy-status" role="status">{feedback.message}</span>
        {/if}
      {/if}
      <figcaption>
        <strong>{asset.modelId}</strong>
        <span>{asset.execution === "cloud" ? "Cloud" : "Local"} · {asset.providerId}</span>
        {#if asset.width && asset.height && asset.byteSize}
          <span>{asset.width}×{asset.height} · {Math.ceil(asset.byteSize / 1024)} KiB PNG</span>
        {/if}
        {#if asset.sources.length > 0}
          <details class="generated-image-lineage">
            <summary>{imageEditingLineageLabel(asset.sources.length)}</summary>
            <ol aria-label="Edit sources">
              {#each asset.sources as source}
                {@const sourcePresentation = imageEditingSourcePresentation(source)}
                <li>
                  <strong>{sourcePresentation.kind}</strong>
                  <span>{sourcePresentation.details}</span>
                </li>
              {/each}
            </ol>
          </details>
        {/if}
      </figcaption>
    </figure>
  {/each}
</div>
{#if responseStored && assets.every((asset) => asset.status === "failed" || asset.status === "cancelled")}
  <div class="message-actions generated-image-actions">
    <button
      class="retry-response"
      aria-label="Retry image generation"
      disabled={isGenerating}
      onclick={() => onretryimage(responseId)}><Icon name="refresh" size={15} /><span>Retry image</span></button
    >
  </div>
{/if}
