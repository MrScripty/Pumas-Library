import type { ReactNode } from 'react';

interface ModelNotesMarkdownPreviewProps {
  markdown: string;
}

interface MarkdownBlockResult {
  block: ReactNode | null;
  nextIndex: number;
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

function isSpecialLine(line: string): boolean {
  return /^#{1,3}\s+/.test(line) || /^[-*]\s+/.test(line) || /^>\s?/.test(line) || /^```/.test(line);
}

function collectCodeBlock(lines: string[], startIndex: number, key: number): MarkdownBlockResult {
  const codeLines: string[] = [];
  let index = startIndex + 1;

  while (index < lines.length) {
    const currentLine = lines[index] ?? '';
    if (/^```/.test(currentLine)) {
      break;
    }
    codeLines.push(currentLine);
    index += 1;
  }

  const nextIndex = index < lines.length ? index + 1 : index;
  return {
    block: (
      <pre
        key={`md-code-${key}`}
        className="overflow-x-auto whitespace-pre-wrap rounded-md border border-[hsl(var(--border-default))] bg-[hsl(var(--surface-high)/0.6)] p-3 font-mono text-sm text-[hsl(var(--text-secondary))]"
      >
        <code>{codeLines.join('\n')}</code>
      </pre>
    ),
    nextIndex,
  };
}

function getHeadingClassName(level: number): string {
  if (level === 1) {
    return 'text-lg font-semibold text-[hsl(var(--text-primary))]';
  }
  if (level === 2) {
    return 'text-base font-semibold text-[hsl(var(--text-primary))]';
  }
  return 'text-sm font-semibold uppercase tracking-wide text-[hsl(var(--text-primary))]';
}

function renderHeadingBlock(line: string, key: number): ReactNode | null {
  const headingMatch = line.match(/^(#{1,3})\s+(.*)$/);
  if (!headingMatch) {
    return null;
  }

  const headingHashes = headingMatch[1] ?? '#';
  const content = headingMatch[2] ?? '';
  return (
    <div key={`md-heading-${key}`} className={getHeadingClassName(headingHashes.length)}>
      {renderMarkdownInline(content, `md-heading-${key}`)}
    </div>
  );
}

function collectListBlock(lines: string[], startIndex: number, key: number): MarkdownBlockResult {
  const items: string[] = [];
  let index = startIndex;

  while (index < lines.length) {
    const currentLine = lines[index] ?? '';
    if (!/^[-*]\s+/.test(currentLine)) {
      break;
    }
    items.push(currentLine.replace(/^[-*]\s+/, ''));
    index += 1;
  }

  return {
    block: (
      <ul key={`md-list-${key}`} className="list-disc space-y-1 pl-5 text-sm text-[hsl(var(--text-secondary))]">
        {items.map((item, itemIndex) => (
          <li key={`md-list-${key}-${itemIndex}`}>
            {renderMarkdownInline(item, `md-list-${key}-${itemIndex}`)}
          </li>
        ))}
      </ul>
    ),
    nextIndex: index,
  };
}

function collectQuoteBlock(lines: string[], startIndex: number, key: number): MarkdownBlockResult {
  const quoteLines: string[] = [];
  let index = startIndex;

  while (index < lines.length) {
    const currentLine = lines[index] ?? '';
    if (!/^>\s?/.test(currentLine)) {
      break;
    }
    quoteLines.push(currentLine.replace(/^>\s?/, ''));
    index += 1;
  }

  return {
    block: (
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
    ),
    nextIndex: index,
  };
}

function collectParagraphBlock(lines: string[], startIndex: number, key: number): MarkdownBlockResult {
  const paragraphLines: string[] = [];
  let index = startIndex;

  while (index < lines.length) {
    const currentLine = lines[index] ?? '';
    if (!currentLine.trim() || isSpecialLine(currentLine)) {
      break;
    }
    paragraphLines.push(currentLine.trim());
    index += 1;
  }

  return {
    block: (
      <p key={`md-paragraph-${key}`} className="text-sm leading-6 text-[hsl(var(--text-secondary))]">
        {renderMarkdownInline(paragraphLines.join(' '), `md-paragraph-${key}`)}
      </p>
    ),
    nextIndex: index,
  };
}

function renderMarkdownBlock(lines: string[], index: number, key: number): MarkdownBlockResult {
  const line = lines[index] ?? '';

  if (!line.trim()) {
    return { block: null, nextIndex: index + 1 };
  }

  if (/^```/.test(line)) {
    return collectCodeBlock(lines, index, key);
  }

  const headingBlock = renderHeadingBlock(line, key);
  if (headingBlock) {
    return { block: headingBlock, nextIndex: index + 1 };
  }

  if (/^[-*]\s+/.test(line)) {
    return collectListBlock(lines, index, key);
  }

  if (/^>\s?/.test(line)) {
    return collectQuoteBlock(lines, index, key);
  }

  return collectParagraphBlock(lines, index, key);
}

export function ModelNotesMarkdownPreview({
  markdown,
}: ModelNotesMarkdownPreviewProps) {
  const normalized = markdown.replace(/\r\n/g, '\n');
  const lines = normalized.split('\n');
  const blocks: ReactNode[] = [];
  let index = 0;
  let key = 0;

  while (index < lines.length) {
    const result = renderMarkdownBlock(lines, index, key);
    if (result.block) {
      blocks.push(result.block);
      key += 1;
    }
    index = result.nextIndex;
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
