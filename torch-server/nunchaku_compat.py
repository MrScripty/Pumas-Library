"""Adapt the pinned Nunchaku 1.2 forward call to Diffusers 0.37."""

from diffusers.models.transformers.transformer_z_image import ZImageTransformer2DModel
from nunchaku import NunchakuZImageTransformer2DModel
from nunchaku.models.transformers.transformer_zimage import NunchakuZImageRopeHook


class PumasNunchakuZImageTransformer(NunchakuZImageTransformer2DModel):
    """Keep Nunchaku's packed rotary hooks and bind Diffusers options by name.

    Nunchaku 1.2's positional super-call predates Diffusers' ControlNet inputs.
    The pinned dependency pair otherwise shares the same Z-Image architecture.
    """

    def forward(self, x, t, cap_feats, patch_size=2, f_patch_size=1, return_dict=True):
        self.register_rope_hook(NunchakuZImageRopeHook())
        try:
            return ZImageTransformer2DModel.forward(
                self,
                x,
                t,
                cap_feats,
                patch_size=patch_size,
                f_patch_size=f_patch_size,
                return_dict=return_dict,
            )
        finally:
            self.unregister_rope_hook()
