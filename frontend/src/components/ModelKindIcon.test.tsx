import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { ModelKindIcon } from './ModelKindIcon';

describe('ModelKindIcon', () => {
  it('labels depth-oriented tasks with a depth token', () => {
    render(<ModelKindIcon kind="depth-estimation" />);

    expect(screen.getByLabelText('Depth')).toBeInTheDocument();
  });

  it('labels segmentation tasks with a mask token', () => {
    render(<ModelKindIcon kind="image-segmentation" />);

    expect(screen.getByLabelText('Mask')).toBeInTheDocument();
  });

  it('labels detection tasks with a detection token', () => {
    render(<ModelKindIcon kind="zero-shot-object-detection" />);

    expect(screen.getByLabelText('Detection')).toBeInTheDocument();
  });
});
