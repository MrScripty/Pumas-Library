import { render, screen, within } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { ModelNotesMarkdownPreview } from './ModelNotesMarkdownPreview';

describe('ModelNotesMarkdownPreview', () => {
  it('renders supported markdown blocks and inline formatting', () => {
    const markdown = [
      '# Model notes',
      '',
      'Paragraph with **bold text**, *emphasis*, `code`, and [link](https://example.test).',
      '',
      '- first item',
      '- second item',
      '',
      '> quoted line',
      '',
      '```',
      'const value = 1;',
      '```',
    ].join('\n');

    const { container } = render(<ModelNotesMarkdownPreview markdown={markdown} />);

    expect(screen.getByText('Model notes')).toBeInTheDocument();
    expect(screen.getByText('bold text').tagName).toBe('STRONG');
    expect(screen.getByText('emphasis').tagName).toBe('EM');
    expect(screen.getByText('code').tagName).toBe('CODE');
    expect(screen.getByRole('link', { name: 'link' })).toHaveAttribute('href', 'https://example.test');
    expect(screen.getByText('first item')).toBeInTheDocument();
    expect(screen.getByText('second item')).toBeInTheDocument();
    expect(screen.getByText('quoted line')).toBeInTheDocument();
    const codeBlock = container.querySelector('pre');
    expect(codeBlock).toBeInstanceOf(HTMLElement);
    if (codeBlock instanceof HTMLElement) {
      expect(within(codeBlock).getByText('const value = 1;')).toBeInTheDocument();
    }
  });

  it('shows an empty notes fallback for blank markdown', () => {
    render(<ModelNotesMarkdownPreview markdown={' \n\n'} />);

    expect(screen.getByText('No notes yet.')).toBeInTheDocument();
  });
});
