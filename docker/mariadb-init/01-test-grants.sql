-- SPDX-License-Identifier: GPL-3.0-only
-- Development and integration-test grants.
--
-- stackarr-core's TestDb harness creates a randomly named database per test
-- (`stackarr_test_<uuid>`) and drops it afterwards, so the account named in
-- TEST_DATABASE_URL needs CREATE/DROP on databases it does not own yet. The
-- MARIADB_USER created by the image is only scoped to MARIADB_DATABASE, which
-- is not enough.
--
-- This file is mounted into /docker-entrypoint-initdb.d and therefore only
-- runs for the local development stack. Do not reuse it for production.

GRANT ALL PRIVILEGES ON *.* TO 'stackarr'@'%';
FLUSH PRIVILEGES;
