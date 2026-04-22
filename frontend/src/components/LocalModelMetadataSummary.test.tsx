import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { LocalModelMetadataSummary } from './LocalModelMetadataSummary';

describe('LocalModelMetadataSummary', () => {
  it('renders model format, quant, size, and dependency metadata', () => {
    render(
      <LocalModelMetadataSummary
        format="gguf"
        quant="Q4_K_M"
        size={1024 ** 3}
        hasDependencies={true}
        dependencyCount={2}
      />
    );

    expect(screen.getByText('GGUF')).toBeInTheDocument();
    expect(screen.getByText('Q4_K_M')).toBeInTheDocument();
    expect(screen.getByText('1.00 GB')).toBeInTheDocument();
    expect(screen.getByText('Deps')).toHaveAttribute(
      'title',
      '2 dependency bindings'
    );
  });

  it('renders explicit fallbacks for missing format, quant, and size', () => {
    render(<LocalModelMetadataSummary />);

    expect(screen.getAllByText('N/A')).toHaveLength(2);
    expect(screen.getByText('Unknown')).toBeInTheDocument();
    expect(screen.queryByText('Deps')).not.toBeInTheDocument();
  });

  it('renders partial download errors near the metadata row', () => {
    render(
      <LocalModelMetadataSummary
        partialError="Download checksum failed."
      />
    );

    expect(screen.getByText('Download checksum failed.')).toBeInTheDocument();
  });
});
