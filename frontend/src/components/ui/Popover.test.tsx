import { useState } from 'react';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

const { motionDivPropsMock, useReducedMotionMock } = vi.hoisted(() => ({
  motionDivPropsMock: vi.fn<(props: Record<string, unknown>) => void>(),
  useReducedMotionMock: vi.fn<() => boolean>(),
}));

vi.mock('framer-motion', async (importOriginal) => {
  const actual = await importOriginal<typeof import('framer-motion')>();
  const React = await import('react');
  const CapturingMotionDiv = React.forwardRef<
    HTMLDivElement,
    React.ComponentProps<typeof actual.motion.div>
  >((props, ref) => {
    motionDivPropsMock(props);
    return React.createElement(actual.motion.div, { ...props, ref });
  });
  CapturingMotionDiv.displayName = 'CapturingMotionDiv';

  return {
    ...actual,
    motion: { div: CapturingMotionDiv },
    useReducedMotion: useReducedMotionMock,
  };
});

import { ModalDialog } from './ModalDialog';
import { Popover } from './Popover';

function PopoverHarness() {
  const [isOpen, setIsOpen] = useState(false);
  return (
    <>
      <Popover
        contentClassName="popup"
        isOpen={isOpen}
        label="Test actions"
        onOpenChange={setIsOpen}
        trigger={(triggerProps) => (
          <button type="button" {...triggerProps}>Open actions</button>
        )}
      >
        <button type="button" onClick={() => setIsOpen(false)}>Choose action</button>
        <button type="button">Last action</button>
      </Popover>
      <button type="button">Outside action</button>
    </>
  );
}

function NestedPopoverHarness() {
  const [isOuterOpen, setIsOuterOpen] = useState(false);
  const [isInnerOpen, setIsInnerOpen] = useState(false);
  return (
    <Popover
      isOpen={isOuterOpen}
      label="Outer actions"
      onOpenChange={setIsOuterOpen}
      trigger={(triggerProps) => (
        <button type="button" {...triggerProps}>Open outer actions</button>
      )}
    >
      <Popover
        isOpen={isInnerOpen}
        label="Inner actions"
        onOpenChange={setIsInnerOpen}
        trigger={(triggerProps) => (
          <button type="button" {...triggerProps}>Open inner actions</button>
        )}
      >
        <button type="button">Inner action</button>
      </Popover>
    </Popover>
  );
}

function ModalPopoverHarness() {
  const [isModalOpen, setIsModalOpen] = useState(false);
  const [isPopoverOpen, setIsPopoverOpen] = useState(false);

  return (
    <>
      <button type="button" onClick={() => setIsModalOpen(true)}>Open modal workflow</button>
      <ModalDialog
        ariaLabel="Composed dialog"
        isOpen={isModalOpen}
        onClose={() => setIsModalOpen(false)}
      >
        <Popover
          isOpen={isPopoverOpen}
          label="Composed popup"
          onOpenChange={setIsPopoverOpen}
          trigger={(triggerProps) => (
            <button type="button" {...triggerProps}>Open composed popup</button>
          )}
        >
          <button type="button">Popup action</button>
        </Popover>
        <button type="button">Modal action</button>
      </ModalDialog>
    </>
  );
}

describe('Popover', () => {
  it.each([
    { reduced: false, expectedOffset: -6, label: 'normal motion' },
    { reduced: true, expectedOffset: 0, label: 'reduced motion' },
  ])('selects $label entry and exit translation', ({ reduced, expectedOffset }) => {
    motionDivPropsMock.mockClear();
    useReducedMotionMock.mockReturnValue(reduced);
    render(<PopoverHarness />);

    fireEvent.click(screen.getByRole('button', { name: 'Open actions' }));

    const popoverCall = motionDivPropsMock.mock.calls
      .map(([props]) => props)
      .find((props) => props['role'] === 'dialog' && props['aria-label'] === 'Test actions');
    expect(popoverCall).toBeDefined();
    expect(popoverCall?.['initial']).toEqual({ opacity: 0, y: expectedOffset });
    expect(popoverCall?.['exit']).toEqual({ opacity: 0, y: expectedOffset });
    expect(popoverCall?.['animate']).toEqual({ opacity: 1, y: 0 });
  });

  it('owns the trigger relationship, focus entry, Escape dismissal, and return', async () => {
    render(<PopoverHarness />);
    const trigger = screen.getByRole('button', { name: 'Open actions' });
    expect(trigger).toHaveAttribute('aria-expanded', 'false');
    expect(trigger).toHaveAttribute('aria-haspopup', 'dialog');

    trigger.focus();
    fireEvent.click(trigger);
    const panel = screen.getByRole('dialog', { name: 'Test actions' });
    expect(trigger).toHaveAttribute('aria-expanded', 'true');
    expect(trigger).toHaveAttribute('aria-controls', panel.id);
    await waitFor(() => expect(screen.getByRole('button', { name: 'Choose action' })).toHaveFocus());

    fireEvent.keyDown(document, { key: 'Escape' });
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Test actions' })).not.toBeInTheDocument());
    expect(trigger).toHaveFocus();
  });

  it('dismisses outside and removes its listeners after cleanup', async () => {
    const listenerRemoval = vi.spyOn(document, 'removeEventListener');
    render(<PopoverHarness />);
    const trigger = screen.getByRole('button', { name: 'Open actions' });
    trigger.focus();
    fireEvent.click(trigger);
    await screen.findByRole('dialog', { name: 'Test actions' });

    fireEvent.mouseDown(screen.getByRole('button', { name: 'Outside action' }));
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Test actions' })).not.toBeInTheDocument());
    expect(trigger).toHaveFocus();
    expect(listenerRemoval).toHaveBeenCalledWith('keydown', expect.any(Function), true);
    expect(listenerRemoval).toHaveBeenCalledWith('mousedown', expect.any(Function), true);
    expect(listenerRemoval).toHaveBeenCalledWith('touchstart', expect.any(Function), true);
    listenerRemoval.mockRestore();
  });

  it('lets only the topmost nested popover respond to Escape', async () => {
    render(<NestedPopoverHarness />);
    const outerTrigger = screen.getByRole('button', { name: 'Open outer actions' });
    outerTrigger.focus();
    fireEvent.click(outerTrigger);
    const innerTrigger = await screen.findByRole('button', { name: 'Open inner actions' });
    fireEvent.click(innerTrigger);
    await screen.findByRole('dialog', { name: 'Inner actions' });

    fireEvent.keyDown(document, { key: 'Escape' });
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Inner actions' })).not.toBeInTheDocument());
    expect(screen.getByRole('dialog', { name: 'Outer actions' })).toBeInTheDocument();
    expect(innerTrigger).toHaveFocus();

    fireEvent.keyDown(document, { key: 'Escape' });
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Outer actions' })).not.toBeInTheDocument());
    expect(outerTrigger).toHaveFocus();
  });

  it('coordinates Escape and restoration when a popover is composed inside a modal', async () => {
    render(<ModalPopoverHarness />);
    const modalTrigger = screen.getByRole('button', { name: 'Open modal workflow' });
    modalTrigger.focus();
    fireEvent.click(modalTrigger);
    const popoverTrigger = await screen.findByRole('button', { name: 'Open composed popup' });
    await waitFor(() => expect(popoverTrigger).toHaveFocus());

    fireEvent.click(popoverTrigger);
    await waitFor(() => expect(screen.getByRole('button', { name: 'Popup action' })).toHaveFocus());
    fireEvent.keyDown(document, { key: 'Escape' });

    await waitFor(() =>
      expect(screen.queryByRole('dialog', { name: 'Composed popup' })).not.toBeInTheDocument()
    );
    expect(screen.getByRole('dialog', { name: 'Composed dialog' })).toBeInTheDocument();
    expect(popoverTrigger).toHaveFocus();

    fireEvent.keyDown(document, { key: 'Escape' });
    await waitFor(() =>
      expect(screen.queryByRole('dialog', { name: 'Composed dialog' })).not.toBeInTheDocument()
    );
    expect(modalTrigger).toHaveFocus();
  });
});
