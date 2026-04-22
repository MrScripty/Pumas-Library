import { useCallback, useState } from 'react';

export function useModelManagerFilters() {
  const [searchQuery, setSearchQuery] = useState('');
  const [selectedCategory, setSelectedCategory] = useState<string>('all');
  const [selectedKind, setSelectedKind] = useState<string>('all');
  const [showCategoryMenu, setShowCategoryMenu] = useState(false);
  const [isDownloadMode, setIsDownloadMode] = useState(false);

  const isCategoryFiltered = isDownloadMode ? selectedKind !== 'all' : selectedCategory !== 'all';
  const hasLocalFilters = Boolean(searchQuery.trim()) || selectedCategory !== 'all';
  const selectedFilter = isDownloadMode ? selectedKind : selectedCategory;

  const clearLocalFilters = useCallback(() => {
    setSearchQuery('');
    setSelectedCategory('all');
  }, []);

  const clearRemoteFilters = useCallback(() => {
    setSearchQuery('');
    setSelectedKind('all');
  }, []);

  const searchDeveloper = useCallback((developer: string) => {
    setIsDownloadMode(true);
    setSearchQuery(developer);
    setSelectedKind('all');
    setShowCategoryMenu(false);
  }, []);

  const toggleMode = useCallback(() => {
    setIsDownloadMode((prev) => !prev);
    setShowCategoryMenu(false);
  }, []);

  const toggleFilterMenu = useCallback(() => {
    setShowCategoryMenu((prev) => !prev);
  }, []);

  const selectFilter = useCallback((item: string) => {
    if (isDownloadMode) {
      setSelectedKind(item);
    } else {
      setSelectedCategory(item);
    }
    setShowCategoryMenu(false);
  }, [isDownloadMode]);

  return {
    clearLocalFilters,
    clearRemoteFilters,
    hasLocalFilters,
    isCategoryFiltered,
    isDownloadMode,
    searchDeveloper,
    searchQuery,
    selectedCategory,
    selectedFilter,
    selectedKind,
    selectFilter,
    setSearchQuery,
    showCategoryMenu,
    toggleFilterMenu,
    toggleMode,
  };
}
