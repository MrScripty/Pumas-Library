/**
 * HuggingFace Authentication Dialog
 *
 * Modal dialog for managing HuggingFace authentication tokens.
 * Supports setting, clearing, and validating tokens against the HF API.
 */

import { useState, useEffect, useCallback } from 'react';
import { AnimatePresence, motion } from 'framer-motion';
import { Key, CheckCircle, XCircle, ExternalLink, Loader2, X } from 'lucide-react';
import { api, isAPIAvailable } from '../api/adapter';
import { getLogger } from '../utils/logger';

const logger = getLogger('HuggingFaceAuthDialog');

export interface HuggingFaceAuthDialogProps {
  isOpen: boolean;
  onClose: () => void;
}

interface AuthState {
  authenticated: boolean;
  username?: string;
  tokenSource?: string;
}

export function HuggingFaceAuthDialog({ isOpen, onClose }: HuggingFaceAuthDialogProps): JSX.Element | null {
  const [authState, setAuthState] = useState<AuthState | null>(null);
  const [tokenInput, setTokenInput] = useState('');
  const [isLoading, setIsLoading] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const fetchAuthStatus = useCallback(async (): Promise<void> => {
    if (!isAPIAvailable()) return;

    setIsLoading(true);
    setError(null);
    try {
      const result = await api.get_hf_auth_status();
      setAuthState({
        authenticated: result.authenticated,
        username: result.username,
        tokenSource: result.token_source,
      });
    } catch (err) {
      logger.error('Failed to fetch HF auth status', { error: err });
      setError('Failed to check authentication status.');
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => {
    if (isOpen) {
      void fetchAuthStatus();
      setTokenInput('');
      setError(null);
    }
  }, [isOpen, fetchAuthStatus]);

  const handleSaveToken = async (): Promise<void> => {
    if (!isAPIAvailable() || !tokenInput.trim()) return;

    setIsSaving(true);
    setError(null);
    try {
      const result = await api.set_hf_token(tokenInput.trim());
      // Clear token from React state immediately (security)
      setTokenInput('');
      if (result.success) {
        // Re-fetch status from backend (no optimistic updates)
        await fetchAuthStatus();
      } else {
        setError(result.error || 'Failed to save token.');
      }
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Failed to save token.';
      setError(message);
    } finally {
      setIsSaving(false);
    }
  };

  const handleClearToken = async (): Promise<void> => {
    if (!isAPIAvailable()) return;

    setIsSaving(true);
    setError(null);
    try {
      const result = await api.clear_hf_token();
      if (result.success) {
        await fetchAuthStatus();
      } else {
        setError(result.error || 'Failed to clear token.');
      }
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Failed to clear token.';
      setError(message);
    } finally {
      setIsSaving(false);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent): void => {
    if (e.key === 'Enter' && tokenInput.trim() && !isSaving) {
      void handleSaveToken();
    }
  };

  if (!isOpen) return null;

  return (
    <AnimatePresence>
      {isOpen && (
        <motion.div
          className="fixed inset-0 z-50 flex items-center justify-center"
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
        >
          {/* Backdrop */}
          <button
            type="button"
            className="absolute inset-0 bg-black/60 backdrop-blur-sm"
            onClick={onClose}
            aria-label="Close dialog"
          />

          {/* Dialog */}
          <motion.div
            className="relative w-full max-w-md mx-4 rounded-xl bg-[hsl(var(--launcher-bg-primary))] border border-[hsl(var(--launcher-border)/0.5)] shadow-2xl overflow-hidden"
            initial={{ scale: 0.95, opacity: 0 }}
            animate={{ scale: 1, opacity: 1 }}
            exit={{ scale: 0.95, opacity: 0 }}
            transition={{ type: 'spring', duration: 0.3 }}
          >
            {/* Header */}
            <div className="flex items-center justify-between px-5 py-4 border-b border-[hsl(var(--launcher-border)/0.3)]">
              <div className="flex items-center gap-2.5">
                <Key className="w-4.5 h-4.5 text-[hsl(var(--accent-primary))]" />
                <h2 className="text-sm font-semibold text-[hsl(var(--launcher-text-primary))]">
                  HuggingFace Authentication
                </h2>
              </div>
              <button
                onClick={onClose}
                className="p-1 rounded-md text-[hsl(var(--launcher-text-muted))] hover:text-[hsl(var(--launcher-text-primary))] hover:bg-[hsl(var(--launcher-bg-secondary))] transition-colors"
              >
                <X className="w-4 h-4" />
              </button>
            </div>

            {/* Body */}
            <div className="px-5 py-4 space-y-4">
              {isLoading ? (
                <div className="flex items-center justify-center py-6">
                  <Loader2 className="w-5 h-5 animate-spin text-[hsl(var(--launcher-text-muted))]" />
                </div>
              ) : authState?.authenticated ? (
                /* Authenticated state */
                <div className="space-y-4">
                  <div className="flex items-center gap-3 px-3 py-3 rounded-lg bg-[hsl(var(--accent-success)/0.1)] border border-[hsl(var(--accent-success)/0.2)]">
                    <CheckCircle className="w-5 h-5 text-[hsl(var(--accent-success))] shrink-0" />
                    <div>
                      <p className="text-sm font-medium text-[hsl(var(--launcher-text-primary))]">
                        Authenticated{authState.username ? ` as ${authState.username}` : ''}
                      </p>
                      {authState.tokenSource && (
                        <p className="text-xs text-[hsl(var(--launcher-text-muted))] mt-0.5">
                          Token source: {authState.tokenSource}
                        </p>
                      )}
                    </div>
                  </div>

                  <p className="text-xs text-[hsl(var(--launcher-text-muted))]">
                    You can access gated models that you have been granted access to on HuggingFace.
                  </p>

                  <button
                    onClick={() => void handleClearToken()}
                    disabled={isSaving}
                    className="w-full px-3 py-2 text-xs font-medium rounded-lg transition-colors bg-[hsl(var(--launcher-bg-secondary))] text-[hsl(var(--launcher-text-primary))] hover:bg-[hsl(var(--launcher-bg-secondary)/0.8)] border border-[hsl(var(--launcher-border)/0.3)] disabled:opacity-50"
                  >
                    {isSaving ? (
                      <Loader2 className="w-3.5 h-3.5 animate-spin mx-auto" />
                    ) : (
                      'Sign Out'
                    )}
                  </button>
                </div>
              ) : (
                /* Unauthenticated state */
                <div className="space-y-4">
                  <div className="flex items-center gap-3 px-3 py-3 rounded-lg bg-[hsl(var(--launcher-bg-secondary)/0.5)] border border-[hsl(var(--launcher-border)/0.3)]">
                    <XCircle className="w-5 h-5 text-[hsl(var(--launcher-text-muted))] shrink-0" />
                    <p className="text-sm text-[hsl(var(--launcher-text-muted))]">
                      Not authenticated. Some gated models require a HuggingFace token.
                    </p>
                  </div>

                  <div className="space-y-2">
                    <label htmlFor="hf-token" className="text-xs font-medium text-[hsl(var(--launcher-text-muted))]">
                      Access Token
                    </label>
                    <input
                      id="hf-token"
                      type="password"
                      value={tokenInput}
                      onChange={(e) => setTokenInput(e.target.value)}
                      onKeyDown={handleKeyDown}
                      placeholder="hf_..."
                      className="w-full text-sm px-3 py-2 rounded-lg bg-[hsl(var(--launcher-bg-secondary))] border border-[hsl(var(--launcher-border)/0.3)] text-[hsl(var(--launcher-text-primary))] placeholder:text-[hsl(var(--launcher-text-muted)/0.5)] focus:outline-none focus:border-[hsl(var(--accent-primary)/0.5)]"
                    />
                  </div>

                  <button
                    type="button"
                    onClick={() => {
                      if (isAPIAvailable()) {
                        void api.open_url('https://huggingface.co/settings/tokens');
                      } else {
                        window.open('https://huggingface.co/settings/tokens', '_blank', 'noopener');
                      }
                    }}
                    className="inline-flex items-center gap-1.5 text-xs text-[hsl(var(--accent-primary))] hover:underline"
                  >
                    <ExternalLink className="w-3 h-3" />
                    Get a token from HuggingFace
                  </button>

                  <button
                    onClick={() => void handleSaveToken()}
                    disabled={isSaving || !tokenInput.trim()}
                    className="w-full px-3 py-2 text-xs font-medium rounded-lg transition-colors bg-[hsl(var(--accent-primary))] text-white hover:bg-[hsl(var(--accent-primary)/0.8)] disabled:opacity-50 disabled:cursor-not-allowed"
                  >
                    {isSaving ? (
                      <Loader2 className="w-3.5 h-3.5 animate-spin mx-auto" />
                    ) : (
                      'Save Token'
                    )}
                  </button>
                </div>
              )}

              {/* Error */}
              {error && (
                <p className="text-xs text-[hsl(var(--accent-error))]">{error}</p>
              )}
            </div>
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
