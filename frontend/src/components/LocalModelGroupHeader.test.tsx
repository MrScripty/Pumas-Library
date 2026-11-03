import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { LocalModelGroupHeader } from './LocalModelGroupHeader';

describe('LocalModelGroupHeader', () => {
  it('renders the group category and model count', () => {
    render(
      <LocalModelGroupHeader
        category="llm"
        modelCount={3}
      />
    );

    expect(screen.getByText('Language models')).toBeInTheDocument();
    expect(screen.getByText('(3)')).toBeInTheDocument();
  });

  it('names new categories and keeps unrecognized ones readable', () => {
    const { rerender } = render(<LocalModelGroupHeader category="video" modelCount={1} />);
    expect(screen.getByText('Video generation')).toBeInTheDocument();
    rerender(<LocalModelGroupHeader category="decision" modelCount={1} />);
    expect(screen.getByText('Decision and evaluation')).toBeInTheDocument();
    rerender(<LocalModelGroupHeader category="future-kind" modelCount={1} />);
    expect(screen.getByText('future-kind')).toBeInTheDocument();
  });
});
