import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { MappingPreviewDialog } from './MappingPreviewDialog';

const {
  getCrossFilesystemWarningMock,
  getSandboxInfoMock,
  isApiAvailableMock,
} = vi.hoisted(() => ({
  getCrossFilesystemWarningMock: vi.fn(),
  getSandboxInfoMock: vi.fn(),
  isApiAvailableMock: vi.fn<() => boolean>(),
}));

vi.mock('../api/adapter', () => ({
  api: {
    get_cross_filesystem_warning: getCrossFilesystemWarningMock,
    get_sandbox_info: getSandboxInfoMock,
  },
  isAPIAvailable: isApiAvailableMock,
}));

vi.mock('./MappingPreview', () => ({
  MappingPreview: () => <div>Mapping preview content</div>,
}));

describe('MappingPreviewDialog', () => {
  it('renders as a named dialog and closes from backdrop or Escape key', () => {
    const onClose = vi.fn();
    isApiAvailableMock.mockReturnValue(true);
    getCrossFilesystemWarningMock.mockResolvedValue({
      success: true,
      cross_filesystem: false,
    });
    getSandboxInfoMock.mockResolvedValue({
      success: true,
      is_sandboxed: false,
    });

    render(
      <MappingPreviewDialog
        isOpen={true}
        versionTag="v1.2.3"
        onClose={onClose}
      />
    );

    expect(screen.getByRole('dialog', { name: 'Sync Library Models' })).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Close mapping preview dialog' }));
    expect(onClose).toHaveBeenCalledTimes(1);

    fireEvent.keyDown(window, { key: 'Escape' });
    expect(onClose).toHaveBeenCalledTimes(2);
  });
});
