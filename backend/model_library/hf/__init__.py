"""HuggingFace operations for model library."""  # pragma: no cover

from __future__ import annotations  # pragma: no cover

from backend.model_library.hf.cache import (  # pragma: no cover
    HFMetadataCache,
    hf_metadata_cache,
    make_cache_key,
)
from backend.model_library.hf.client import HfClient  # pragma: no cover
from backend.model_library.hf.formats import (  # pragma: no cover
    KNOWN_FORMATS,
    extract_formats,
    extract_formats_from_paths,
)
from backend.model_library.hf.metadata import (  # pragma: no cover
    KIND_TAG_MAPPING,
    coerce_int,
    collect_paths_with_sizes,
    infer_kind_from_tags,
)
from backend.model_library.hf.quant import (  # pragma: no cover
    QUANT_TOKENS,
    extract_quants_from_paths,
    normalize_quant_source,
    quant_sizes_from_paths,
    sorted_quants,
    token_in_normalized,
)
from backend.model_library.hf.search import list_repo_tree_paths, search_models  # pragma: no cover
from backend.model_library.hf.throttle import HFAPIThrottle, hf_throttle  # pragma: no cover

__all__ = [  # pragma: no cover
    "HfClient",
    "HFAPIThrottle",
    "HFMetadataCache",
    "KIND_TAG_MAPPING",
    "KNOWN_FORMATS",
    "QUANT_TOKENS",
    "coerce_int",
    "collect_paths_with_sizes",
    "extract_formats",
    "extract_formats_from_paths",
    "extract_quants_from_paths",
    "hf_metadata_cache",
    "hf_throttle",
    "infer_kind_from_tags",
    "list_repo_tree_paths",
    "make_cache_key",
    "normalize_quant_source",
    "quant_sizes_from_paths",
    "search_models",
    "sorted_quants",
    "token_in_normalized",
]
