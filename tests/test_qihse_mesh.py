"""The display probe must never hammer AIMEM with invalid AUTH credentials."""
import importlib.util
import os
from pathlib import Path
from types import SimpleNamespace
from unittest import TestCase, mock
import tempfile

SOURCE = Path(__file__).resolve().parents[1] / 'examples/plugins/qihse_mesh.py'
spec = importlib.util.spec_from_file_location('qihse_mesh_test_module', SOURCE)
mesh = importlib.util.module_from_spec(spec)
spec.loader.exec_module(mesh)


class QihseMeshTests(TestCase):
    def setUp(self):
        mesh._aimem_session = None
        mesh._aimem_retry_after = 0.0

    def test_legacy_env_cannot_send_plaintext_auth(self):
        with tempfile.TemporaryDirectory() as td:
            env_file = Path(td) / 'env'
            env_file.write_text('QIHSE_AUTH_USER=GODMODE_OP\n')
            env_file.chmod(0o600)
            with (mock.patch.dict(os.environ, {'QIHSE_AIMEM_ENV': str(env_file)}),
                  mock.patch.object(mesh.socket, 'socket') as sock):
                self.assertIsNone(mesh.probe_local_redis())
                sock.assert_not_called()

    def test_failed_qkp_auth_has_backoff(self):
        with tempfile.TemporaryDirectory() as td:
            td = Path(td)
            sdk, lib = td / 'sdk.py', td / 'libqihse.so'
            sdk.touch(); lib.touch()
            env_file = td / 'env'
            env_file.write_text(''.join(f'{k}={v}\n' for k, v in {
                'QIHSE_AIMEM_TRANSPORT': 'qkp', 'QIHSE_AUTH_USER': 'AIMEM_SERVICE',
                'QIHSE_AUTH_PASSWORD': 'test-only', 'QIHSE_ADMIN_SDK': str(sdk),
                'QIHSE_ADMIN_LIB': str(lib), 'QIHSE_AIMEM_IDENTITY_DIR': str(td),
                'QIHSE_AIMEM_TRUSTED_PUB': str(td / 'pub.pem'),
                'QIHSE_AIMEM_NODE_ID': 'display-test',
            }.items()))
            env_file.chmod(0o600)
            attempted = []
            class FakeAdmin:
                is_native = True
                _lib = SimpleNamespace(_qihse_lib_path=str(lib))
                def __init__(self, **_): pass
                def session(self, *_, **__):
                    attempted.append(1)
                    raise RuntimeError('bad test credential')
            fake_spec = SimpleNamespace(loader=SimpleNamespace(exec_module=lambda _: None))
            with (mock.patch.dict(os.environ, {'QIHSE_AIMEM_ENV': str(env_file)}),
                  mock.patch.object(mesh.importlib.util, 'spec_from_file_location', return_value=fake_spec),
                  mock.patch.object(mesh.importlib.util, 'module_from_spec',
                                    return_value=SimpleNamespace(ClusterAdmin=FakeAdmin)),
                  mock.patch.object(mesh.socket, 'socket') as sock):
                self.assertIsNone(mesh.probe_local_redis())
                self.assertIsNone(mesh.probe_local_redis())
                self.assertEqual(len(attempted), 1)
                sock.assert_not_called()
