import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { Tooltip } from './Tooltip';

describe('Tooltip', () => {
  it('describes its trigger through aria-describedby while closed', () => {
    render(
      <Tooltip content="Load model">
        <button type="button" aria-describedby="existing-hint">Load</button>
      </Tooltip>
    );
    const button = screen.getByRole('button', { name: 'Load' });
    const tooltip = screen.getByRole('tooltip', { hidden: true });

    expect(tooltip).not.toBeVisible();
    expect(button.getAttribute('aria-describedby')?.split(' ')).toEqual(['existing-hint', tooltip.id]);
    expect(button).toHaveAccessibleDescription('Load model');
  });

  it('opens on keyboard focus, closes on Escape and on blur', async () => {
    const user = userEvent.setup();
    const onParentKeyDown = vi.fn();
    render(
      // eslint-disable-next-line jsx-a11y/no-static-element-interactions -- probe for propagation only
      <div onKeyDown={onParentKeyDown}>
        <Tooltip content="Unload model">
          <button type="button">Unload</button>
        </Tooltip>
        <button type="button">Next</button>
      </div>
    );

    await user.tab();
    expect(screen.getByRole('button', { name: 'Unload' })).toHaveFocus();
    expect(screen.getByRole('tooltip')).toHaveTextContent('Unload model');

    await user.keyboard('{Escape}');
    expect(screen.queryByRole('tooltip')).not.toBeInTheDocument();
    expect(onParentKeyDown).toHaveBeenCalled();

    await user.tab();
    await user.tab({ shift: true });
    expect(screen.getByRole('tooltip')).toBeVisible();

    await user.tab();
    expect(screen.queryByRole('tooltip')).not.toBeInTheDocument();
  });

  it('opens on hover and closes when the pointer leaves', async () => {
    const user = userEvent.setup();
    render(
      <Tooltip content="Remove from Ollama">
        <button type="button">Remove</button>
      </Tooltip>
    );

    await user.hover(screen.getByRole('button', { name: 'Remove' }));
    expect(screen.getByRole('tooltip')).toHaveTextContent('Remove from Ollama');

    await user.unhover(screen.getByRole('button', { name: 'Remove' }));
    expect(screen.queryByRole('tooltip')).not.toBeInTheDocument();
  });

  it('renders no tooltip element for empty content', () => {
    render(
      <Tooltip content="">
        <button type="button">Plain</button>
      </Tooltip>
    );
    expect(screen.queryByRole('tooltip', { hidden: true })).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Plain' })).not.toHaveAttribute('aria-describedby');
  });
});
