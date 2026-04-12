import { Eye, PencilLine } from 'lucide-react';
import { ModelNotesMarkdownPreview } from './ModelNotesMarkdownPreview';

interface ModelNotesEditorProps {
  notesDraft: string;
  notesPreview: boolean;
  notesSaving: boolean;
  notesDirty: boolean;
  notesSaveError: string | null;
  notesSaveSuccess: boolean;
  onNotesDraftChange: (value: string) => void;
  onNotesPreviewChange: (preview: boolean) => void;
  onSaveNotes: () => void;
  onRevertNotes: () => void;
}

export function ModelNotesEditor({
  notesDraft,
  notesPreview,
  notesSaving,
  notesDirty,
  notesSaveError,
  notesSaveSuccess,
  onNotesDraftChange,
  onNotesPreviewChange,
  onSaveNotes,
  onRevertNotes,
}: ModelNotesEditorProps) {
  return (
    <div className="space-y-4">
      <div className="flex items-start justify-between gap-3">
        <div>
          <div className="text-sm font-medium text-[hsl(var(--text-primary))]">User Notes</div>
          <div className="text-xs text-[hsl(var(--text-muted))]">
            Markdown notes stored in `metadata.json`. Use this for your own annotations about the model.
          </div>
        </div>
        <div className="flex shrink-0 items-center gap-2">
          <button
            onClick={() => onNotesPreviewChange(false)}
            className={`flex items-center gap-1 rounded px-2.5 py-1 text-xs ${
              !notesPreview
                ? 'bg-[hsl(var(--launcher-accent-primary)/0.2)] text-[hsl(var(--text-primary))]'
                : 'bg-[hsl(var(--surface-high))] text-[hsl(var(--text-secondary))] hover:bg-[hsl(var(--surface-mid))]'
            }`}
          >
            <PencilLine className="h-3.5 w-3.5" />
            Edit
          </button>
          <button
            onClick={() => onNotesPreviewChange(true)}
            className={`flex items-center gap-1 rounded px-2.5 py-1 text-xs ${
              notesPreview
                ? 'bg-[hsl(var(--launcher-accent-primary)/0.2)] text-[hsl(var(--text-primary))]'
                : 'bg-[hsl(var(--surface-high))] text-[hsl(var(--text-secondary))] hover:bg-[hsl(var(--surface-mid))]'
            }`}
          >
            <Eye className="h-3.5 w-3.5" />
            Preview
          </button>
        </div>
      </div>

      {notesPreview ? (
        <div className="min-h-64 max-h-80 overflow-y-auto rounded-lg border border-[hsl(var(--border-default))] bg-[hsl(var(--surface-high)/0.35)] p-4">
          <ModelNotesMarkdownPreview markdown={notesDraft} />
        </div>
      ) : (
        <textarea
          value={notesDraft}
          onChange={(event) => onNotesDraftChange(event.target.value)}
          placeholder={'# Notes\n\nWrite markdown notes about this model here.\n\n- strengths\n- caveats\n- prompt tips'}
          spellCheck={false}
          className="min-h-64 max-h-80 w-full resize-y rounded-lg border border-[hsl(var(--border-default))] bg-[hsl(var(--surface-high)/0.35)] px-3 py-3 font-mono text-sm leading-6 text-[hsl(var(--text-primary))]"
        />
      )}

      <div className="flex items-center gap-3 border-t border-[hsl(var(--border-default))] pt-2">
        <button
          onClick={onSaveNotes}
          disabled={notesSaving || !notesDirty}
          className="rounded bg-[hsl(var(--launcher-accent-primary))] px-4 py-1.5 text-sm text-white hover:opacity-90 disabled:opacity-50"
        >
          {notesSaving ? 'Saving...' : 'Save Notes'}
        </button>
        <button
          onClick={onRevertNotes}
          disabled={notesSaving || !notesDirty}
          className="rounded bg-[hsl(var(--surface-high))] px-3 py-1.5 text-sm text-[hsl(var(--text-secondary))] hover:bg-[hsl(var(--surface-mid))] disabled:opacity-40"
        >
          Revert
        </button>
        {notesSaveSuccess && (
          <span className="text-xs text-[hsl(var(--accent-success))]">Saved</span>
        )}
        {notesSaveError && (
          <span className="text-xs text-[hsl(var(--accent-error))]">{notesSaveError}</span>
        )}
      </div>
    </div>
  );
}
