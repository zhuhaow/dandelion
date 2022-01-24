# Changelog

## Unreleased

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

