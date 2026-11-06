import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import type { ModelLibraryDiagnostics } from '../hooks/useModels';
import { ModelLibraryStatusNotice } from './ModelLibraryStatusNotice';

const clean: ModelLibraryDiagnostics = { loadError: null, scanError: null, searchError: null, snapshotWrittenAt: null };

describe('ModelLibraryStatusNotice', () => {
  it('renders nothing for a ready library without failures', () => {
    const { container } = render(
      <ModelLibraryStatusNotice libraryLoadStatus="ready" diagnostics={clean} hasVisibleModels downloadListError={null} />
    );
    expect(container).toBeEmptyDOMElement();
  });

  it('shows the backend reason when the library is unavailable', () => {
    render(
      <ModelLibraryStatusNotice
        libraryLoadStatus="unavailable"
        diagnostics={{ ...clean, loadError: 'Library database is locked' }}
        hasVisibleModels
        downloadListError={null}
      />
    );
    expect(screen.getByRole('alert')).toHaveTextContent(
      'Library unavailable. Library database is locked Showing previously loaded models.'
    );
  });

  it('labels cold-start rows with the time the snapshot was written', () => {
    const writtenAt = '2026-11-04T09:00:00.000Z';
    render(
      <ModelLibraryStatusNotice
        libraryLoadStatus="loading"
        diagnostics={{ ...clean, snapshotWrittenAt: writtenAt }}
        hasVisibleModels
        downloadListError={null}
      />
    );
    expect(screen.getByRole('status')).toHaveTextContent(
      `Loading library… Showing models cached at ${new Date(writtenAt).toLocaleString()}.`
    );
  });

  it('keeps scan, search and download-list failures visible while the library is ready', () => {
    render(
      <ModelLibraryStatusNotice
        libraryLoadStatus="ready"
        diagnostics={{ ...clean, scanError: 'Models root is not readable', searchError: 'Index unavailable' }}
        hasVisibleModels={false}
        downloadListError="Download registry unreadable"
      />
    );
    const alerts = screen.getAllByRole('alert').map((alert) => alert.textContent);
    expect(alerts).toEqual([
      'Library scan failed: Models root is not readable',
      'Library search failed: Index unavailable',
      'Download list unavailable: Download registry unreadable',
    ]);
  });
});
