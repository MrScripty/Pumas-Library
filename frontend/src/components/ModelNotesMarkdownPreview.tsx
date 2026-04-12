import type { ReactNode } from 'react';

interface ModelNotesMarkdownPreviewProps {
  markdown: string;
}

function renderMarkdownInline(text: string, keyPrefix: string): ReactNode[] {
  const pattern = /(\[[^\]]+\]\(([^)\s]+)\)|`([^`]+)`|\*\*([^*]+)\*\*|\*([^*]+)\*)/;
  const nodes: ReactNode[] = [];
  let remaining = text;
  let index = 0;

  while (remaining.length > 0) {
    const match = remaining.match(pattern);
    if (!match) {
      nodes.push(<span key={`${keyPrefix}-text-${index}`}>{remaining}</span>);
      break;
    }

    const offset = match.index ?? 0;
    if (offset > 0) {
      nodes.push(
        <span key={`${keyPrefix}-text-${index}`}>
          {remaining.slice(0, offset)}
        </span>
      );
      index += 1;
    }

    const [fullMatch, , linkUrl, codeText, strongText, emphasisText] = match;
    if (linkUrl) {
      const label = fullMatch.slice(1, fullMatch.indexOf(']'));
      nodes.push(
        <a
          key={`${keyPrefix}-link-${index}`}
          href={linkUrl}
          target="_blank"
          rel="noopener noreferrer"
          className="text-[hsl(var(--accent-link))] hover:underline"
        >
          {label}
        </a>
      );
    } else if (typeof codeText === 'string') {
      nodes.push(
        <code
          key={`${keyPrefix}-code-${index}`}
          className="rounded bg-[hsl(var(--surface-high))] px-1 py-0.5 font-mono text-[0.95em] text-[hsl(var(--text-primary))]"
        >
          {codeText}
        </code>
      );
    } else if (typeof strongText === 'string') {
      nodes.push(
        <strong
          key={`${keyPrefix}-strong-${index}`}
          className="font-semibold text-[hsl(var(--text-primary))]"
        >
          {strongText}
        </strong>
      );
    } else if (typeof emphasisText === 'string') {
      nodes.push(
        <em key={`${keyPrefix}-em-${index}`} className="italic">
          {emphasisText}
        </em>
      );
    }

    remaining = remaining.slice(offset + fullMatch.length);
    index += 1;
  }

  return nodes;
}

export function ModelNotesMarkdownPreview({
  markdown,
}: ModelNotesMarkdownPreviewProps) {
  const normalized = markdown.replace(/\r\n/g, '\n');
  const lines = normalized.split('\n');
  const blocks: ReactNode[] = [];
  let index = 0;
  let key = 0;

  const isSpecialLine = (line: string) =>
    /^#{1,3}\s+/.test(line) || /^[-*]\s+/.test(line) || /^>\s?/.test(line) || /^```/.test(line);

  while (index < lines.length) {
    const line = lines[index] ?? '';

    if (!line.trim()) {
      index += 1;
      continue;
    }

    if (/^```/.test(line)) {
      const codeLines: string[] = [];
      index += 1;
      while (index < lines.length) {
        const currentLine = lines[index] ?? '';
        if (/^```/.test(currentLine)) {
          break;
        }
        codeLines.push(currentLine);
        index += 1;
      }
      if (index < lines.length) {
        index += 1;
      }
      blocks.push(
        <pre
          key={`md-code-${key}`}
          className="overflow-x-auto whitespace-pre-wrap rounded-md border border-[hsl(var(--border-default))] bg-[hsl(var(--surface-high)/0.6)] p-3 font-mono text-sm text-[hsl(var(--text-secondary))]"
        >
          <code>{codeLines.join('\n')}</code>
        </pre>
      );
      key += 1;
      continue;
    }

    const headingMatch = line.match(/^(#{1,3})\s+(.*)$/);
    if (headingMatch) {
      const headingHashes = headingMatch[1] ?? '#';
      const content = headingMatch[2] ?? '';
      const level = headingHashes.length;
      const className = level === 1
        ? 'text-lg font-semibold text-[hsl(var(--text-primary))]'
        : level === 2
          ? 'text-base font-semibold text-[hsl(var(--text-primary))]'
          : 'text-sm font-semibold uppercase tracking-wide text-[hsl(var(--text-primary))]';
      blocks.push(
        <div key={`md-heading-${key}`} className={className}>
          {renderMarkdownInline(content, `md-heading-${key}`)}
        </div>
      );
      key += 1;
      index += 1;
      continue;
    }

    if (/^[-*]\s+/.test(line)) {
      const items: string[] = [];
      while (index < lines.length) {
        const currentLine = lines[index] ?? '';
        if (!/^[-*]\s+/.test(currentLine)) {
          break;
        }
        items.push(currentLine.replace(/^[-*]\s+/, ''));
        index += 1;
      }
      blocks.push(
        <ul key={`md-list-${key}`} className="list-disc space-y-1 pl-5 text-sm text-[hsl(var(--text-secondary))]">
          {items.map((item, itemIndex) => (
            <li key={`md-list-${key}-${itemIndex}`}>
              {renderMarkdownInline(item, `md-list-${key}-${itemIndex}`)}
            </li>
          ))}
        </ul>
      );
      key += 1;
      continue;
    }

    if (/^>\s?/.test(line)) {
      const quoteLines: string[] = [];
      while (index < lines.length) {
        const currentLine = lines[index] ?? '';
        if (!/^>\s?/.test(currentLine)) {
          break;
        }
        quoteLines.push(currentLine.replace(/^>\s?/, ''));
        index += 1;
      }
      blocks.push(
        <blockquote
          key={`md-quote-${key}`}
          className="space-y-1 border-l-2 border-[hsl(var(--border-default))] pl-3 text-sm italic text-[hsl(var(--text-muted))]"
        >
          {quoteLines.map((quoteLine, quoteIndex) => (
            <div key={`md-quote-${key}-${quoteIndex}`}>
              {renderMarkdownInline(quoteLine, `md-quote-${key}-${quoteIndex}`)}
            </div>
          ))}
        </blockquote>
      );
      key += 1;
      continue;
    }

    const paragraphLines: string[] = [];
    while (index < lines.length) {
      const currentLine = lines[index] ?? '';
      if (!currentLine.trim() || isSpecialLine(currentLine)) {
        break;
      }
      paragraphLines.push(currentLine.trim());
      index += 1;
    }
    blocks.push(
      <p key={`md-paragraph-${key}`} className="text-sm leading-6 text-[hsl(var(--text-secondary))]">
        {renderMarkdownInline(paragraphLines.join(' '), `md-paragraph-${key}`)}
      </p>
    );
    key += 1;
  }

  if (blocks.length === 0) {
    return (
      <div className="text-sm text-[hsl(var(--text-muted))]">
        No notes yet.
      </div>
    );
  }

  return <div className="space-y-3">{blocks}</div>;
}
