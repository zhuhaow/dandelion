//
//  ProxyHelper.swift
//  me.zhuhaow.Specht2.proxy-helper
//
//  Created by Zhuhao Wang on 2021/12/23.
//

import Foundation
import SystemConfiguration

class ProxyHelper: NSObject {
    let authRef: AuthorizationRef

    override init() {
        var auth: AuthorizationRef?
        let authFlags: AuthorizationFlags = [.extendRights, .interactionAllowed, .preAuthorize]

        let authErr = AuthorizationCreate(nil, nil, authFlags, &auth)

        if authErr != noErr {
            NSLog("Error: Failed to create administration authorization due to error \(authErr).")
        }

        if auth == nil {
            NSLog("Error: No authorization has been granted to modify network configuration.")
        }

        authRef = auth!

        super.init()
    }

    deinit {
        AuthorizationFree(authRef, AuthorizationFlags())
    }

    func updateSocks5Proxy(endpoint: Endpoint?) throws {
        try updateConfigure(type: kSCNetworkProtocolTypeProxies) { dic in
            if let endpoint = endpoint {
                dic[kCFNetworkProxiesSOCKSProxy as String] = endpoint.connectableAddr as AnyObject
                dic[kCFNetworkProxiesSOCKSEnable as String] = 1 as AnyObject
                dic[kCFNetworkProxiesSOCKSPort as String] = endpoint.port as AnyObject
            } else {
                dic[kCFNetworkProxiesSOCKSProxy as String] = "" as AnyObject
                dic[kCFNetworkProxiesSOCKSEnable as String] = 0 as AnyObject
                dic[kCFNetworkProxiesSOCKSPort as String] = 0 as AnyObject
            }
        }
    }

    func updateHttpProxy(endpoint: Endpoint?) throws {
        try updateConfigure(type: kSCNetworkProtocolTypeProxies) { dic in
            if let endpoint = endpoint {
                dic[kCFNetworkProxiesHTTPProxy as String] = endpoint.connectableAddr as AnyObject
                dic[kCFNetworkProxiesHTTPEnable as String] = 1 as AnyObject
                dic[kCFNetworkProxiesHTTPSProxy as String] = endpoint.connectableAddr as AnyObject
                dic[kCFNetworkProxiesHTTPSEnable as String] = 1 as AnyObject
                dic[kCFNetworkProxiesHTTPSPort as String] = endpoint.port as AnyObject
                dic[kCFNetworkProxiesHTTPPort as String] = endpoint.port as AnyObject
            } else {
                dic[kCFNetworkProxiesHTTPProxy as String] = "" as AnyObject
                dic[kCFNetworkProxiesHTTPEnable as String] = 0 as AnyObject
                dic[kCFNetworkProxiesHTTPSProxy as String] = "" as AnyObject
                dic[kCFNetworkProxiesHTTPSEnable as String] = 0 as AnyObject
                dic[kCFNetworkProxiesHTTPSPort as String] = 0 as AnyObject
                dic[kCFNetworkProxiesHTTPPort as String] = 0 as AnyObject
            }
        }
    }

    func updateDns(endpoint: Endpoint?) throws {
        try updateConfigure(type: kSCNetworkProtocolTypeDNS) { dic in
            if let endpoint = endpoint {
                dic[kSCPropNetDNSServerAddresses] = [endpoint.connectableAddr] as NSArray as AnyObject
            } else {
                dic.removeObject(forKey: kSCPropNetDNSServerAddresses)
            }
        }
    }

    func flushDnsCache() throws {
        let process = Process()

        process.arguments = ["-HUP", "mDNSResponder"]
        process.executableURL = URL(fileURLWithPath: "/usr/bin/killall")

        try process.run()
        process.waitUntilExit()

        guard process.terminationStatus == EXIT_SUCCESS else {
            throw XpcError("Failed to refresh DNS cache, exit with \(process.terminationStatus)")
        }
    }

    private func updateConfigure(type: CFString, with: @escaping (inout NSMutableDictionary) -> Void) throws {
        guard let prefRef = SCPreferencesCreateWithAuthorization(nil,
                                                                 Bundle.main.bundleIdentifier! as CFString,
                                                                 nil, authRef) else {
            NSLog("Error: Failed to obtain preference ref.")
            return
        }

        guard SCPreferencesLock(prefRef, true) else {
            NSLog("Error: Failed to obtain lock to preference.")
            return
        }

        defer {
            SCPreferencesUnlock(prefRef)
        }

        guard let networks = SCNetworkSetCopyCurrent(prefRef),
              let services = SCNetworkSetCopyServices(networks) as? [SCNetworkService] else {
            NSLog("Error: Failed to load network services.")
            return
        }

        let ethernetServices = services.filter {
            SCNetworkServiceGetEnabled($0) && {
                guard let interface = SCNetworkServiceGetInterface($0) else {
                    return false
                }

                // Everyone is using Wi-Fi nowadays, right?
                return SCNetworkInterfaceGetInterfaceType(interface) == kSCNetworkInterfaceTypeIEEE80211
            }($0)
        }

        guard !ethernetServices.isEmpty else {
            NSLog("Error: Failed to find active ethernet network service.")
            return
        }

        for service in ethernetServices {
            guard let protoc = SCNetworkServiceCopyProtocol(service, type) else {
                NSLog("Error: Failed to obtain \(type) settings for \(SCNetworkServiceGetName(service)!)")
                continue
            }

            let config = SCNetworkProtocolGetConfiguration(protoc)

            var dic = (config as NSDictionary?)?.mutableCopy() as? NSMutableDictionary ?? NSMutableDictionary()
            with(&dic)

            guard SCNetworkProtocolSetConfiguration(protoc, dic as CFDictionary) else {
                NSLog("Error: Failed to set \(type) settings for \(SCNetworkServiceGetName(service)!)")
                continue
            }

            NSLog("Set \(type) settings for \(SCNetworkServiceGetName(service)!)")
        }

        guard SCPreferencesCommitChanges(prefRef) else {
            NSLog("Error: Failed to commit preference change")
            return
        }

        guard SCPreferencesApplyChanges(prefRef) else {
            NSLog("Error: Failed to apply preference change")
            return
        }
    }
}
