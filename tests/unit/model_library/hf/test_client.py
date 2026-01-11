"""Tests for HuggingFace client wrapper."""

from __future__ import annotations

from unittest.mock import MagicMock, patch

import pytest

from backend.model_library.hf.client import HfClient


@pytest.mark.unit
def test_hf_client_init_with_token():
    """Test HfClient initialization with explicit token."""
    client = HfClient(token="test_token")
    assert client.hf_token == "test_token"
    assert client._api is None


@pytest.mark.unit
def test_hf_client_init_from_env(monkeypatch):
    """Test HfClient initialization from environment variable."""
    monkeypatch.setenv("HF_TOKEN", "env_token")
    client = HfClient()
    assert client.hf_token == "env_token"
    assert client._api is None


@pytest.mark.unit
def test_hf_client_init_no_token(monkeypatch):
    """Test HfClient initialization without token."""
    monkeypatch.delenv("HF_TOKEN", raising=False)
    client = HfClient()
    assert client.hf_token is None
    assert client._api is None


@pytest.mark.unit
def test_get_api_caches_instance():
    """Test that get_api caches the API instance."""
    with patch("huggingface_hub.HfApi") as mock_api_class:
        with patch("huggingface_hub.login") as mock_login:
            mock_api_instance = MagicMock()
            mock_api_class.return_value = mock_api_instance

            client = HfClient(token="test_token")
            api1 = client.get_api()
            api2 = client.get_api()

            assert api1 is api2
            assert api1 is mock_api_instance
            mock_api_class.assert_called_once()
            mock_login.assert_called_once_with("test_token")


@pytest.mark.unit
def test_get_api_without_token():
    """Test get_api without token does not call login."""
    with patch("huggingface_hub.HfApi") as mock_api_class:
        with patch("huggingface_hub.login") as mock_login:
            mock_api_instance = MagicMock()
            mock_api_class.return_value = mock_api_instance

            client = HfClient()
            api = client.get_api()

            assert api is mock_api_instance
            mock_api_class.assert_called_once()
            mock_login.assert_not_called()


@pytest.mark.unit
def test_get_api_import_error():
    """Test get_api raises RuntimeError when huggingface_hub not installed."""
    with patch.dict("sys.modules", {"huggingface_hub": None}):
        client = HfClient()
        with pytest.raises(RuntimeError, match="huggingface_hub is not installed"):
            client.get_api()


@pytest.mark.unit
def test_get_api_login_called_with_token():
    """Test that login is called when token is provided."""
    with patch("huggingface_hub.HfApi") as mock_api_class:
        with patch("huggingface_hub.login") as mock_login:
            mock_api_instance = MagicMock()
            mock_api_class.return_value = mock_api_instance

            client = HfClient(token="my_secret_token")
            api = client.get_api()

            assert api is mock_api_instance
            mock_login.assert_called_once_with("my_secret_token")
            mock_api_class.assert_called_once()
