//
//  Proxy.swift
//  Specht2
//
//  Created by Zhuhao Wang on 2021/12/24.
//

import Foundation
import ServiceManagement

enum ProxyError: Error {
    case authorizationError(OSStatus)
    case blessError(String)
}

class Endpoint {
    let addr: String
    let port: UInt16

    var connectableAddr: String {
        if addr == "0.0.0.0" {
            return "127.0.0.1"
        } else {
            return addr
        }
    }

    init(addr: String, port: UInt16) {
        self.addr = addr
        self.port = port
    }
}

class Proxy {
    static func setProxy(socks5: Endpoint?, http: Endpoint?) {
        XpcConnection.runCommandWithLatestXpcService { result in
            switch result {
            case .success(let proxy):
                proxy.setProxy(setSocks5: socks5 != nil,
                               socks5Address: socks5?.connectableAddr ?? "",
                               socks5Port: socks5?.port ?? 0,
                                setHttp: http != nil,
                               httpAddress: http?.connectableAddr ?? "",
                               httpPort: http?.port ?? 0)
            case .failure(let error):
                Alert.alert(message: "Failed to set up system proxy: \(error)")
            }

        }
    }
}

private class XpcConnection {
    private static var _connection: NSXPCConnection?

    private static func getDefaultConnection(block: @escaping (Result<NSXPCConnection, Error>) -> Void) {
        if let conn = _connection {
            block(.success(conn))
            return
        }

        let connection = createConnection()

        // swiftlint:disable force_cast
        let remoteProxy = connection.remoteObjectProxyWithErrorHandler { error in
            if let err = error as NSError?,
               err.domain == NSCocoaErrorDomain && err.code == NSXPCConnectionInvalid {
                // The XPC service is not available, install it.
                    switch bless() {
                    case .success:
                        _connection = createConnection()
                        // we don't have to check version since it should be the latest
                        block(.success(connection))
                    case .failure(let err):
                        block(.failure(err))
                    }
                    return
            }
            block(.failure(error))
        } as! ProxyHelperInterface

        remoteProxy.currentVersion { version in
            if version == Constants.version {
                _connection = connection
                block(.success(connection))
            } else {
                switch bless() {
                case .success:
                    _connection = createConnection()
                    block(.success(connection))
                case .failure(let err):
                    block(.failure(err))
                }
            }
        }
    }

    static func runCommandWithLatestXpcService(block: @escaping (Result<ProxyHelperInterface, Error>) -> Void) {
        getDefaultConnection { result in
            block(result.map { conn in
                // swiftlint:disable force_cast
                return conn.remoteObjectProxyWithErrorHandler { error in
                    block(.failure(error))
                } as! ProxyHelperInterface
            })
        }
    }

    private static func createConnection() -> NSXPCConnection {
        let connection = NSXPCConnection(machServiceName: Constants.helperMachLabel,
                                         options: .privileged)

        connection.remoteObjectInterface = NSXPCInterface(with: ProxyHelperInterface.self)

        connection.invalidationHandler = {
            _connection?.invalidationHandler = nil
            _connection = nil
        }

        connection.resume()

        return connection
    }

    private static func bless() -> Result<Void, ProxyError> {
        var authorization: AuthorizationRef?

        let status: OSStatus = AuthorizationCreate(nil, nil, [], &authorization)
        guard status == errAuthorizationSuccess else {
            return .failure(.authorizationError(status))
        }

        var error: Unmanaged<CFError>?
        let blessStatus = SMJobBless(kSMDomainSystemLaunchd,
                                     Constants.helperMachLabel as CFString,
                                     authorization, &error)

        if !blessStatus {
            return .failure(.blessError(error!.takeRetainedValue().localizedDescription))
        }
        return .success(())
    }
}
