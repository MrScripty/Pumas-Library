import { useRef, useState } from 'react';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { ModalDialog } from './ModalDialog';

function NestedDialogs() {
  const [isOuterOpen, setIsOuterOpen] = useState(false);
  const [isInnerOpen, setIsInnerOpen] = useState(false);
  const outerInitialFocusRef = useRef<HTMLButtonElement>(null);
  const innerInitialFocusRef = useRef<HTMLButtonElement>(null);

  return (
    <>
      <button type="button" onClick={() => setIsOuterOpen(true)}>Open outer</button>
      <ModalDialog
        ariaLabel="Outer dialog"
        initialFocusRef={outerInitialFocusRef}
        isOpen={isOuterOpen}
        onClose={() => setIsOuterOpen(false)}
      >
        <button ref={outerInitialFocusRef} type="button">Outer first</button>
        <button type="button" onClick={() => setIsInnerOpen(true)}>Open inner</button>
        <button type="button">Outer last</button>
        <ModalDialog
          ariaLabel="Inner dialog"
          initialFocusRef={innerInitialFocusRef}
          isOpen={isInnerOpen}
          onClose={() => setIsInnerOpen(false)}
        >
          <button ref={innerInitialFocusRef} type="button">Inner first</button>
          <button type="button" onClick={() => setIsOuterOpen(false)}>Close all dialogs</button>
          <button type="button">Inner last</button>
        </ModalDialog>
      </ModalDialog>
    </>
  );
}

describe('ModalDialog', () => {
  it('uses the task-owned focus destination when refresh removes the opener', async () => {
    function RefreshingDialog() {
      const [open, setOpen] = useState(false);
      const [removed, setRemoved] = useState(false);
      const fallback = useRef<HTMLButtonElement>(null);
      return <>
        <button ref={fallback}>Library</button>
        {!removed && <button onClick={() => setOpen(true)}>Open conversion</button>}
        <ModalDialog isOpen={open} ariaLabel="Conversion" onClose={() => setOpen(false)} restoreFocusFallbackRef={fallback}>
          <button onClick={() => setRemoved(true)}>Refresh library</button>
          <button onClick={() => setOpen(false)}>Close conversion</button>
        </ModalDialog>
      </>;
    }
    render(<RefreshingDialog />);
    const opener = screen.getByRole('button', { name: 'Open conversion' });
    opener.focus();
    fireEvent.click(opener);
    fireEvent.click(screen.getByRole('button', { name: 'Refresh library' }));
    fireEvent.click(screen.getByRole('button', { name: 'Close conversion' }));
    await waitFor(() => expect(screen.getByRole('button', { name: 'Library' })).toHaveFocus());
  });

  it('contains focus in only the topmost modal and restores each connected opener', async () => {
    render(<NestedDialogs />);
    const outerTrigger = screen.getByRole('button', { name: 'Open outer' });
    outerTrigger.focus();
    fireEvent.click(outerTrigger);

    const outerFirst = screen.getByRole('button', { name: 'Outer first' });
    await waitFor(() => expect(outerFirst).toHaveFocus());

    const innerTrigger = screen.getByRole('button', { name: 'Open inner' });
    innerTrigger.focus();
    fireEvent.click(innerTrigger);
    const innerFirst = screen.getByRole('button', { name: 'Inner first' });
    await waitFor(() => expect(innerFirst).toHaveFocus());

    const innerLast = screen.getByRole('button', { name: 'Inner last' });
    innerLast.focus();
    fireEvent.keyDown(document, { key: 'Tab' });
    expect(innerFirst).toHaveFocus();

    outerTrigger.focus();
    expect(innerFirst).toHaveFocus();

    fireEvent.keyDown(document, { key: 'Escape' });
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Inner dialog' })).not.toBeInTheDocument());
    expect(screen.getByRole('dialog', { name: 'Outer dialog' })).toBeInTheDocument();
    expect(innerTrigger).toHaveFocus();

    fireEvent.keyDown(document, { key: 'Escape' });
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Outer dialog' })).not.toBeInTheDocument());
    expect(outerTrigger).toHaveFocus();
  });

  it('applies disabled dismissal to Escape and backdrop while retaining explicit actions', async () => {
    const onClose = vi.fn();
    const initialFocusRef = { current: null };
    render(
      <ModalDialog
        ariaLabel="Busy dialog"
        dismissDisabled={true}
        initialFocusRef={initialFocusRef}
        isOpen={true}
        onClose={onClose}
      >
        <button type="button" onClick={onClose}>Explicit close</button>
      </ModalDialog>
    );

    const dialog = screen.getByRole('dialog', { name: 'Busy dialog' });
    await waitFor(() => expect(screen.getByRole('button', { name: 'Explicit close' })).toHaveFocus());
    fireEvent.keyDown(document, { key: 'Escape' });
    const backdrop = dialog.parentElement?.querySelector<HTMLElement>('[data-modal-backdrop]');
    if (!backdrop) {
      throw new TypeError('Expected modal backdrop');
    }
    fireEvent.mouseDown(backdrop);
    expect(onClose).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole('button', { name: 'Explicit close' }));
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('restores past a parent modal when a nested hierarchy is removed together', async () => {
    render(<NestedDialogs />);
    const outerTrigger = screen.getByRole('button', { name: 'Open outer' });
    outerTrigger.focus();
    fireEvent.click(outerTrigger);
    fireEvent.click(await screen.findByRole('button', { name: 'Open inner' }));
    await waitFor(() => expect(screen.getByRole('button', { name: 'Inner first' })).toHaveFocus());

    fireEvent.click(screen.getByRole('button', { name: 'Close all dialogs' }));
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
    expect(outerTrigger).toHaveFocus();
  });

  it('removes global lifecycle listeners when unmounted', () => {
    const onClose = vi.fn();
    const { unmount } = render(
      <ModalDialog ariaLabel="Temporary dialog" isOpen={true} onClose={onClose}>
        <button type="button">Action</button>
      </ModalDialog>
    );

    unmount();
    fireEvent.keyDown(document, { key: 'Escape' });
    expect(onClose).not.toHaveBeenCalled();
  });
});
