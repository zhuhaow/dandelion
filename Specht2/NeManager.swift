//
//  Manager.swift
//  Extension
//
//  Created by Zhuhao Wang on 2021/11/11.
//

import Foundation
import NetworkExtension

class Manager {
    static func createManager() async throws {
        let managers = try await NETransparentProxyManager.loadAllFromPreferences()
        for manager in managers {
            try await manager.removeFromPreferences()
        }
        
        let manager = NETransparentProxyManager()
        manager.localizedDescription = "Specht2"
        let configuration = NETunnelProviderProtocol()
        configuration.providerBundleIdentifier = Bundle.main.bundleIdentifier! + ".Extension"
        configuration.excludeLocalNetworks = true
        configuration.serverAddress = "127.0.0.1"
        manager.protocolConfiguration = configuration
        
        try await manager.saveToPreferences()
    }
}
