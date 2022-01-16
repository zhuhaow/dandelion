//
//  Service.swift
//  Specht2
//
//  Created by Zhuhao Wang on 2022/1/15.
//

import Foundation
import SwiftyXPC
import ServiceManagement
import System

enum ServiceError: Error {
    case authorizationError(OSStatus)
    case blessError(Error)
}

class Service {
    private static var _service: Service?

    let connection: XPCConnection

    private init(connection: XPCConnection) {
        XPCErrorRegistry.shared.registerDomain(nil, forErrorType: FfiError.self)
        self.connection = connection
    }

    static func getDefaultService() async throws -> Service {
        if let service = _service {
            return service
        }

        do {
            let service = try createService()

            let version = try await service.currentVersion()

            if version == Constants.version {
                _service = service
                return service
            } else {
                try bless()
                let service = try createService()
                _service = service
                return service
            }
        } catch {
            guard let err = error as? XPCError else {
                throw error
            }

            switch err {
            // The service is not properly installed.
            case .connectionInvalid, .connectionInterrupted, .invalidCodeSignatureRequirement:
                try bless()
                let service = try createService()
                _service = service
                return service
            default:
                throw error
            }
        }
    }

    static func createService() throws -> Service {
        let service = Service(connection:
                                try XPCConnection(type: .remoteMachService(serviceName: Constants.helperMachLabel,
                                                                           isPrivilegedHelperTool: true),
                                                  codeSigningRequirement: Constants.serviceCodeSignRequirements))

        service.connection.activate()

        service.connection.errorHandler = { (_, _) in
            _service = nil
        }

        return service
    }

    private static func bless() throws {
        var authorization: AuthorizationRef?

        let status: OSStatus = AuthorizationCreate(nil, nil, [], &authorization)
        guard status == errAuthorizationSuccess else {
            throw ServiceError.authorizationError(status)
        }

        var error: Unmanaged<CFError>?
        let blessStatus = SMJobBless(kSMDomainSystemLaunchd,
                                     Constants.helperMachLabel as CFString,
                                     authorization, &error)

        guard blessStatus else {
            throw ServiceError.blessError(error!.takeRetainedValue())
        }
    }
}

extension Service: ServiceInterface {
    func setSocks5Proxy(endpoint: Endpoint?) async throws {
        try await connection.sendMessage(name: ServiceInterfaceMethodName.setSocks5Proxy, request: endpoint)
    }

    func setHttpProxy(endpoint: Endpoint?) async throws {
        try await connection.sendMessage(name: ServiceInterfaceMethodName.setHttpProxy, request: endpoint)
    }

    func setDns(endpoint: Endpoint?) async throws {
        try await connection.sendMessage(name: ServiceInterfaceMethodName.setDns, request: endpoint)
    }

    func createTunInterface(subnet: String) async throws -> XPCFileDescriptor {
        return try await connection.sendMessage(name: ServiceInterfaceMethodName.createTunInterface, request: subnet)
    }

    func currentVersion() async throws -> String {
        return try await connection.sendMessage(name: ServiceInterfaceMethodName.currentVersion)
    }
}

extension XPCError: LocalizedError {}
