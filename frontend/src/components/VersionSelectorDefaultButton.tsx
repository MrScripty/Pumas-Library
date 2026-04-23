import { useEffect, useState } from 'react';
import { useHover } from '@react-aria/interactions';
import { Anchor, CircleX } from 'lucide-react';
import { APIError } from '../errors';
import { getLogger } from '../utils/logger';

const logger = getLogger('VersionSelectorDropdown');

interface VersionSelectorDefaultButtonProps {
  isDefault: boolean;
  isLoading: boolean;
  isRowHovered: boolean;
  isSwitching: boolean;
  onMakeDefault: (tag: string | null) => Promise<boolean>;
  version: string;
}

function reportDefaultVersionError(error: unknown, version: string, isDefault: boolean): void {
  const action = isDefault ? 'clearing' : 'setting';
  if (error instanceof APIError && error.endpoint) {
    logger.error(`API error ${action} default version`, {
      error: error.message,
      endpoint: error.endpoint,
      version,
    });
  } else if (error instanceof Error) {
    logger.error(`Failed to ${isDefault ? 'clear' : 'set'} default version`, {
      error: error.message,
      version,
    });
  } else {
    logger.error(`Unknown error ${action} default version`, {
      error: String(error),
      version,
    });
  }
}

function DefaultAnchorIcon({
  hoverStartedAsDefault,
  isDefault,
  isHovered,
  isRowHovered,
}: {
  hoverStartedAsDefault: boolean;
  isDefault: boolean;
  isHovered: boolean;
  isRowHovered: boolean;
}) {
  if (isDefault && isHovered && hoverStartedAsDefault) {
    return <CircleX size={14} className="text-[hsl(var(--text-tertiary))]" />;
  }
  if (isDefault) {
    return <Anchor size={14} className="text-[hsl(var(--accent-success))]" />;
  }
  if (isRowHovered) {
    return <Anchor size={14} className="text-[hsl(var(--text-tertiary))]" />;
  }
  return <Anchor size={14} className="text-transparent" />;
}

export function VersionSelectorDefaultButton({
  isDefault,
  isLoading,
  isRowHovered,
  isSwitching,
  onMakeDefault,
  version,
}: VersionSelectorDefaultButtonProps) {
  const { hoverProps, isHovered } = useHover({});
  const [hoverStartedAsDefault, setHoverStartedAsDefault] = useState(false);

  useEffect(() => {
    if (isHovered) {
      setHoverStartedAsDefault(isDefault);
    }
  }, [isHovered, isDefault]);

  return (
    <button
      type="button"
      {...hoverProps}
      onClick={(event) => {
        event.stopPropagation();
        const target = isDefault ? null : version;
        onMakeDefault(target).catch((error: unknown) => {
          reportDefaultVersionError(error, version, isDefault);
        });
      }}
      className="flex items-center justify-center"
      aria-label={isDefault ? `Unset ${version} as default` : `Set ${version} as default`}
      title={isDefault ? 'Click to unset as default' : 'Click to set as default'}
      disabled={isSwitching || isLoading}
    >
      <DefaultAnchorIcon
        hoverStartedAsDefault={hoverStartedAsDefault}
        isDefault={isDefault}
        isHovered={isHovered}
        isRowHovered={isRowHovered}
      />
    </button>
  );
}
