import { useCallback, useState } from 'react';

type UseExistingLibraryChooserOptions = {
  onChooseExistingLibrary?: () => Promise<void> | void;
};

export function useExistingLibraryChooser({
  onChooseExistingLibrary,
}: UseExistingLibraryChooserOptions) {
  const [isChoosingExistingLibrary, setIsChoosingExistingLibrary] = useState(false);

  const chooseExistingLibrary = useCallback(async () => {
    if (!onChooseExistingLibrary || isChoosingExistingLibrary) {
      return;
    }

    setIsChoosingExistingLibrary(true);
    try {
      await onChooseExistingLibrary();
    } finally {
      setIsChoosingExistingLibrary(false);
    }
  }, [isChoosingExistingLibrary, onChooseExistingLibrary]);

  return {
    chooseExistingLibrary,
    isChoosingExistingLibrary,
  };
}
