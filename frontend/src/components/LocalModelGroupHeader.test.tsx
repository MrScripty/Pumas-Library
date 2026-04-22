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

    expect(screen.getByText('llm')).toBeInTheDocument();
    expect(screen.getByText('(3)')).toBeInTheDocument();
  });
});
