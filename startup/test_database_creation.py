"""Database startup must not require maintenance-database access in opt-out mode."""
import os
import sys
import unittest
from unittest.mock import MagicMock, patch

import setup_database_new as setup


class DatabaseCreation(unittest.TestCase):
    def setUp(self):
        self.env = patch.dict(os.environ, {'DB_NAME': 'app_only'}, clear=True)
        self.env.start()
        self.addCleanup(self.env.stop)

    @patch('database_functions.migrations.default_db_type', return_value='postgresql')
    def test_precreated_database_never_connects_to_maintenance_database(self, _):
        with patch.dict(os.environ, {'DB_CREATE_DATABASE': 'false'}), patch.object(setup, 'wait_for_postgresql_ready', return_value=True) as ready:
            with patch.dict(sys.modules, {'psycopg': MagicMock()}) as modules:
                setup.create_database_if_not_exists()
                modules['psycopg'].connect.assert_not_called()
            ready.assert_called_once_with('app_only')

    @patch('database_functions.migrations.default_db_type', return_value='postgresql')
    def test_default_preserves_automatic_creation(self, _):
        driver = MagicMock()
        with patch.dict(sys.modules, {'psycopg': driver}), patch.object(setup, 'wait_for_postgresql_ready', return_value=True):
            setup.create_database_if_not_exists()
        self.assertEqual(driver.connect.call_args.kwargs['dbname'], 'postgres')

    @patch('database_functions.migrations.default_db_type', return_value='postgresql')
    def test_invalid_setting_does_not_fall_back_to_broader_access(self, _):
        with patch.dict(os.environ, {'DB_CREATE_DATABASE': 'flase'}), patch.object(setup, 'wait_for_postgresql_ready') as ready:
            with self.assertRaises(ValueError):
                setup.create_database_if_not_exists()
            ready.assert_not_called()

    @patch('database_functions.migrations.default_db_type', return_value='postgresql')
    def test_missing_precreated_database_does_not_trigger_creation(self, _):
        driver = MagicMock()
        with patch.dict(os.environ, {'DB_CREATE_DATABASE': 'false'}), patch.dict(sys.modules, {'psycopg': driver}), patch.object(setup, 'wait_for_postgresql_ready', return_value=False):
            with self.assertRaisesRegex(Exception, 'did not become ready'):
                setup.create_database_if_not_exists()
        driver.connect.assert_not_called()

    @patch('database_functions.migrations.default_db_type', return_value='sqlite')
    def test_sqlite_startup_remains_unchanged(self, _):
        with patch.dict(os.environ, {'DB_CREATE_DATABASE': 'false'}), patch.object(setup, 'wait_for_postgresql_ready') as ready:
            setup.create_database_if_not_exists()
            ready.assert_not_called()

    def test_readiness_probes_only_the_requested_database(self):
        driver = MagicMock()
        def probe_now(label, host, port, probe):
            probe()
            return True
        with patch.dict(sys.modules, {'psycopg': driver}), patch.object(setup, 'wait_for_database', side_effect=probe_now):
            self.assertTrue(setup.wait_for_postgresql_ready('app_only'))
        self.assertEqual(driver.connect.call_args.kwargs['dbname'], 'app_only')


if __name__ == '__main__':
    unittest.main()
