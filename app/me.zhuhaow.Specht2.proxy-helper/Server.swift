//
//  XpcServer.swift
//  me.zhuhaow.Specht2.proxy-helper
//
//  Created by Zhuhao Wang on 2021/12/23.
//

// Based on https://github.com/aronskaya/smjobbless/blob/master/com.smjobblesssample.installer/XPCServer.swift

import Foundation
import SwiftyXPC
import System

class Server: ServiceInterface {
    let listener: XPCListener
    let proxyHelper = ProxyHelper()

    init() {
        XPCErrorRegistry.shared.registerDomain(nil, forErrorType: FfiError.self)

        // swiftlint:disable force_try
        listener = try! XPCListener(type: .machService(name: Constants.helperMachLabel),
                                    codeSigningRequirement: Constants.clientCodeSignRequirements)

        listener.setMessageHandler(name: ServiceInterfaceMethodName.setSocks5Proxy,
                                   handler: self.setSocks5ProxyWrapper)
        listener.setMessageHandler(name: ServiceInterfaceMethodName.setHttpProxy,
                                   handler: self.setHttpProxyWrapper)
        listener.setMessageHandler(name: ServiceInterfaceMethodName.setDns,
                                   handler: self.setDnsWrapper)
        listener.setMessageHandler(name: ServiceInterfaceMethodName.createTunInterface,
                                   handler: self.createTunInterfaceWrapper)
        listener.setMessageHandler(name: ServiceInterfaceMethodName.currentVersion,
                                   handler: self.currentVersionWrapper)

        listener.activate()
    }

    func setSocks5ProxyWrapper(_: XPCConnection, endpoint: Endpoint?) async throws {
        try await setSocks5Proxy(endpoint: endpoint)
    }

    func setHttpProxyWrapper(_: XPCConnection, endpoint: Endpoint?) async throws {
        try await setHttpProxy(endpoint: endpoint)
    }

    func setDnsWrapper(_: XPCConnection, endpoint: Endpoint?) async throws {
        try await setDns(endpoint: endpoint)
    }

    func createTunInterfaceWrapper(_: XPCConnection, subnet: String) async throws -> XPCFileDescriptor {
        try await createTunInterface(subnet: subnet)
    }

    func currentVersionWrapper(_: XPCConnection) async throws -> String {
        try await currentVersion()
    }

    func setSocks5Proxy(endpoint: Endpoint?) async throws {
        try proxyHelper.updateSocks5Proxy(endpoint: endpoint)
    }

    func setHttpProxy(endpoint: Endpoint?) async throws {
        try proxyHelper.updateHttpProxy(endpoint: endpoint)
    }

    func setDns(endpoint: Endpoint?) async throws {
        try proxyHelper.updateDns(endpoint: endpoint)
    }

    func createTunInterface(subnet: String) async throws -> XPCFileDescriptor {
        NSLog("Creating tun interface for \(subnet)")

        let fileDescriptor = subnet.withCString {
            specht2_create_tun(UnsafeMutablePointer(mutating: $0))
        }

        if fileDescriptor < 0 {
            let error = takeFfiLastError()!
            NSLog("Failed to create tun interface: \(error)")
            throw error
        }

        defer {
            close(fileDescriptor)
        }

        return XPCFileDescriptor(fileDescriptor: fileDescriptor)
    }

    func currentVersion() async throws -> String {
        return Constants.version
    }
}
