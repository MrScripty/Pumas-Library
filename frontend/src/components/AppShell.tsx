import type { ComponentProps } from 'react';
import { Header } from './Header';
import { AppSidebar } from './AppSidebar';
import { ModelImportDropZone } from './ModelImportDropZone';
import { ModelImportDialog } from './ModelImportDialog';
import { AppPanelRenderer } from './app-panels/AppPanelRenderer';

interface AppShellProps {
  header: ComponentProps<typeof Header>;
  importPaths: string[];
  panels: ComponentProps<typeof AppPanelRenderer>;
  showImportDialog: boolean;
  showSidebar: boolean;
  sidebar: ComponentProps<typeof AppSidebar>;
  onImportComplete: () => void;
  onImportDialogClose: () => void;
  onPathsDropped: (paths: string[]) => void;
}

export function AppShell({
  header,
  importPaths,
  panels,
  showImportDialog,
  showSidebar,
  sidebar,
  onImportComplete,
  onImportDialogClose,
  onPathsDropped,
}: AppShellProps) {
  return (
    <div className="w-full h-screen gradient-bg-blobs flex flex-col relative overflow-hidden font-mono">
      <ModelImportDropZone onPathsDropped={onPathsDropped} enabled={true} />

      {showImportDialog && importPaths.length > 0 && (
        <ModelImportDialog
          importPaths={importPaths}
          onClose={onImportDialogClose}
          onImportComplete={onImportComplete}
        />
      )}

      <Header {...header} />

      <div className="flex flex-1 relative z-10 overflow-hidden">
        {showSidebar && <AppSidebar {...sidebar} />}

        <div className="flex-1 flex flex-col overflow-hidden">
          <AppPanelRenderer {...panels} />
        </div>
      </div>
    </div>
  );
}
