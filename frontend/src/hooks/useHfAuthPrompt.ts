import { useCallback, useEffect, useRef, useState } from 'react';

type UseHfAuthPromptOptions = {
  downloadErrors: Record<string, string>;
  isAuthRequiredError: (_errorMessage: string) => boolean;
};

export function useHfAuthPrompt({
  downloadErrors,
  isAuthRequiredError,
}: UseHfAuthPromptOptions) {
  const [isHfAuthOpen, setIsHfAuthOpen] = useState(false);
  const previousDownloadErrorsRef = useRef<Record<string, string>>({});

  useEffect(() => {
    const previousErrors = previousDownloadErrorsRef.current;
    for (const [repoId, errorMessage] of Object.entries(downloadErrors)) {
      if (!previousErrors[repoId] && isAuthRequiredError(errorMessage)) {
        setIsHfAuthOpen(true);
        break;
      }
    }
    previousDownloadErrorsRef.current = downloadErrors;
  }, [downloadErrors, isAuthRequiredError]);

  const openHfAuth = useCallback(() => {
    setIsHfAuthOpen(true);
  }, []);

  const closeHfAuth = useCallback(() => {
    setIsHfAuthOpen(false);
  }, []);

  return {
    closeHfAuth,
    isHfAuthOpen,
    openHfAuth,
  };
}
