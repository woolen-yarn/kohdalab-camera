"""Public acquisition and adapter-extension API."""
from .backends import Camera,Frame,BackendSpec,make_camera,available_backends,register_backend
from .storage import save_frame

__all__=['Camera','Frame','BackendSpec','make_camera','available_backends','register_backend','save_frame']
