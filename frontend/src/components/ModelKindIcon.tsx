/**
 * Model Kind Icon Component
 *
 * Renders icons for model kinds/types.
 * Extracted from ModelManager.tsx
 */

import React from 'react';
import {
  ArrowLeftRight,
  ArrowRight,
  AudioWaveform,
  Box,
  Image,
  Languages,
  Shapes,
  Tag,
  TvMinimalPlay,
} from 'lucide-react';

function resolveKindIcon(token: string) {
  const normalized = token.toLowerCase();
  if (normalized.includes('classification')) return Shapes;
  if (normalized.includes('text')) return Languages;
  if (normalized.includes('image')) return Image;
  if (normalized.includes('audio')) return AudioWaveform;
  if (normalized.includes('video')) return TvMinimalPlay;
  if (normalized.includes('3d')) return Box;
  return Tag;
}

function resolveKindLabel(token: string) {
  const normalized = token.toLowerCase();
  if (normalized.includes('classification')) return 'Classification';
  if (normalized.includes('text')) return 'Text';
  if (normalized.includes('image')) return 'Image';
  if (normalized.includes('audio')) return 'Audio';
  if (normalized.includes('video')) return 'Video';
  if (normalized.includes('3d')) return '3D';
  return 'Unknown';
}

function renderKindToken(token: string) {
  const Icon = resolveKindIcon(token);
  const label = resolveKindLabel(token);
  return (
    <span title={label} aria-label={label} className="inline-flex">
      <Icon className="w-3.5 h-3.5" />
    </span>
  );
}

interface ModelKindIconProps {
  kind: string;
}

export function ModelKindIcon({ kind }: ModelKindIconProps) {
  if (!kind || kind === 'unknown') {
    return (
      <span title="Unknown" aria-label="Unknown" className="inline-flex">
        <Tag className="w-3.5 h-3.5" />
      </span>
    );
  }

  const normalized = kind.toLowerCase();

  // Handle simple kinds (no arrows)
  if (!normalized.includes('-to-')) {
    const tokens = normalized.split('-').filter(Boolean);
    if (tokens.length <= 1) {
      return renderKindToken(normalized);
    }
    return (
      <>
        {tokens.map((token) => (
          <React.Fragment key={token}>{renderKindToken(token)}</React.Fragment>
        ))}
      </>
    );
  }

  // Handle transformation kinds (e.g., "text-to-image")
  const [fromRaw, toRaw] = normalized.split('-to-');
  const fromTokens = (fromRaw || '').split('-').filter(Boolean);
  const toTokens = (toRaw || '').split('-').filter(Boolean);
  const isBidirectional =
    fromTokens.length === toTokens.length &&
    fromTokens.every((token) => toTokens.includes(token));
  const ArrowIcon = isBidirectional ? ArrowLeftRight : ArrowRight;
  const arrowLabel = isBidirectional ? 'Bidirectional' : 'To';

  return (
    <>
      {fromTokens.length > 0
        ? fromTokens.map((token, index) => (
            <React.Fragment key={`from-${token}-${index}`}>
              {renderKindToken(token)}
            </React.Fragment>
          ))
        : renderKindToken(fromRaw || '')}
      <span title={arrowLabel} aria-label={arrowLabel} className="inline-flex">
        <ArrowIcon className="w-3.5 h-3.5 opacity-70" />
      </span>
      {toTokens.length > 0
        ? toTokens.map((token, index) => (
            <React.Fragment key={`to-${token}-${index}`}>
              {renderKindToken(token)}
            </React.Fragment>
          ))
        : renderKindToken(toRaw || '')}
    </>
  );
}
