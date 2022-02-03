# Changelog

## Unreleased

## 0.6.1

Fix issue when system use expired dns result from fake dns server by extending
the clean up delay to 1 hour.

## 0.6.0

Use smarter algorithm to handle TCP flow in tun mode.
Enable keepalive on all TCP connections.

## 0.5.0

**[App]** Add support for logging

## 0.4.0

Use config file to determine if the app should update network configuration.

Fix that domain used for rule matching may be with or without ending dot. Now there is no ending dot.

Fix security issues in dependencies.

## 0.3.1

Add support to use any connector that connects to the same endpoint for the connection pool.

## 0.3.0

Add support for connection pool

## 0.2.0

**[App]** Flush DNS cache when we update system DNS configuration

### Fix

**[App]** Fix file description leak when using tun interface

## 0.1.0

**[App]** Add support for tun.

**[Known Issue]** We can not correctly close tun interface. We need to kill the app and XPC service to bring down the tun interface.

## 0.0.14

**[App]** Fix "too many files" error.

## 0.0.13

**[App]** Fix issue that app may not update proxy settings correctly

## 0.0.12

**[App]** Add support to manage system proxy setting automatically

## 0.0.11

Add support for Happy Eyeball algorithm.

## 0.0.10

Test release with CI

