import React, { useEffect, useState } from 'react';
import { Check, Link2 } from 'lucide-react';

interface AppConnectionInfoProps {
  url: string;
  label?: string;
}

const copyToClipboard = async (text: string) => {
  if (navigator.clipboard?.writeText) {
    await navigator.clipboard.writeText(text);
    return;
  }

  const textarea = document.createElement('textarea');
  textarea.value = text;
  textarea.setAttribute('readonly', 'true');
  textarea.style.position = 'fixed';
  textarea.style.top = '-9999px';
  textarea.style.left = '-9999px';
  document.body.appendChild(textarea);
  textarea.select();
  const success = document.execCommand('copy');
  document.body.removeChild(textarea);

  if (!success) {
    throw new Error('Copy failed');
  }
};

export const AppConnectionInfo: React.FC<AppConnectionInfoProps> = ({ url, label = 'Connection URL' }) => {
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    if (!copied) return;
    const timeout = window.setTimeout(() => setCopied(false), 1800);
    return () => window.clearTimeout(timeout);
  }, [copied]);

  const handleCopy = async () => {
    try {
      await copyToClipboard(url);
      setCopied(true);
    } catch {
      setCopied(false);
    }
  };

  return (
    <div className="w-full max-w-2xl">
      <div className="flex items-center justify-between mb-2 text-xs uppercase tracking-wider text-[hsl(var(--launcher-text-muted))]">
        <span>{label}</span>
        {copied && (
          <span className="text-[hsl(var(--launcher-accent-success))]">Copied</span>
        )}
      </div>
      <button
        type="button"
        onClick={handleCopy}
        className="w-full flex items-center justify-between gap-3 px-4 py-3 rounded-lg border border-[hsl(var(--launcher-border))] bg-[hsl(var(--launcher-bg-secondary)/0.4)] hover:bg-[hsl(var(--launcher-bg-secondary)/0.7)] transition-colors"
        title="Copy URL"
      >
        <div className="flex items-center gap-2 min-w-0">
          <Link2 className="w-4 h-4 text-[hsl(var(--launcher-text-secondary))] flex-shrink-0" />
          <span className="text-sm text-[hsl(var(--launcher-accent-primary))] font-mono truncate">
            {url}
          </span>
        </div>
        <span className="flex items-center gap-1 text-xs text-[hsl(var(--launcher-text-secondary))] flex-shrink-0">
          {copied ? (
            <Check className="w-4 h-4 text-[hsl(var(--launcher-accent-success))]" />
          ) : (
            'Copy'
          )}
        </span>
      </button>
    </div>
  );
};
