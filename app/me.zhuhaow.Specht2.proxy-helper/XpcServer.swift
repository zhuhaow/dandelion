//
//  XpcServer.swift
//  me.zhuhaow.Specht2.proxy-helper
//
//  Created by Zhuhao Wang on 2021/12/23.
//

// Based on https://github.com/aronskaya/smjobbless/blob/master/com.smjobblesssample.installer/XPCServer.swift

import Foundation

class XpcServer: NSObject {

    internal static let shared = XpcServer()

    private var listener: NSXPCListener?

    internal func start() {
        listener = NSXPCListener(machServiceName: Constants.helperMachLabel)
        listener?.delegate = self
        listener?.resume()

        RunLoop.main.run()
    }

    private func connetionInterruptionHandler() {
        NSLog("Connection interrupted")
    }

    private func connectionInvalidationHandler() {
        NSLog("Connection invalidated")
    }

    private func isValidClient(forConnection connection: NSXPCConnection) -> Bool {
        var token = connection.auditToken
        let tokenData = Data(bytes: &token, count: MemoryLayout.size(ofValue: token))
        let attributes = [kSecGuestAttributeAudit: tokenData]

        // Check which flags you need
        let flags: SecCSFlags = []
        var code: SecCode?
        var status = SecCodeCopyGuestWithAttributes(nil, attributes as CFDictionary, flags, &code)

        if status != errSecSuccess {
            return false
        }

        guard let dynamicCode = code else {
            return false
        }

        let entitlements =
            "identifier \"me.zhuhaow.Specht2\"" +
            " and anchor apple generic and certificate leaf[subject.OU] = \"H5443445N6\""
        var requirement: SecRequirement?

        status = SecRequirementCreateWithString(entitlements as CFString, flags, &requirement)

        if status != errSecSuccess {
            return false
        }

        status = SecCodeCheckValidity(dynamicCode, flags, requirement)

        return status == errSecSuccess
    }
}

extension XpcServer: NSXPCListenerDelegate {

    func listener(_ listener: NSXPCListener, shouldAcceptNewConnection newConnection: NSXPCConnection) -> Bool {
        NSLog("Got a new connection")

        if !isValidClient(forConnection: newConnection) {
            NSLog("Client is not valid")
            return false
        }

        newConnection.exportedInterface = NSXPCInterface(with: ProxyHelperInterface.self)
        newConnection.exportedObject = ProxyHelper()

        newConnection.interruptionHandler = connetionInterruptionHandler
        newConnection.invalidationHandler = connectionInvalidationHandler

        newConnection.resume()

        return true
    }
}
