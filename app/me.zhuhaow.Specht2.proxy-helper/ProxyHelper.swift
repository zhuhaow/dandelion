//
//  ProxyHelper.swift
//  me.zhuhaow.Specht2.proxy-helper
//
//  Created by Zhuhao Wang on 2021/12/23.
//

import Foundation
import SystemConfiguration

class ProxyHelper: NSObject, ProxyHelperInterface {
    func setProxy(setSocks5: Bool, socks5Address: String, socks5Port: UInt16,
                  setHttp: Bool, httpAddress: String, httpPort: UInt16) {
        var socksEndpoint: Endpoint?
        var httpEndpoint: Endpoint?

        if setSocks5 {
            socksEndpoint = Endpoint(addr: socks5Address, port: socks5Port)
        }

        if setHttp {
            httpEndpoint = Endpoint(addr: httpAddress, port: httpPort)
        }

        updateProxy(httpProxy: httpEndpoint, socksProxy: socksEndpoint)
    }

    func currentVersion(completionHandler: @escaping (String) -> Void) {
        completionHandler(Constants.version)
    }

    private func updateProxy(httpProxy: Endpoint?, socksProxy: Endpoint?) {
        var authRef: AuthorizationRef?
        let authFlags: AuthorizationFlags = [.extendRights, .interactionAllowed, .preAuthorize]

        let authErr = AuthorizationCreate(nil, nil, authFlags, &authRef)

        guard authErr == noErr else {
            NSLog("Error: Failed to create administration authorization due to error \(authErr).")
            return
        }

        guard authRef != nil else {
            NSLog("Error: No authorization has been granted to modify network configuration.")
            return
        }

        defer {
            AuthorizationFree(authRef!, AuthorizationFlags())
        }

        guard let prefRef = SCPreferencesCreateWithAuthorization(nil, Bundle.main.bundleIdentifier! as CFString, nil, authRef) else {
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

        guard let networks = SCNetworkSetCopyCurrent(prefRef) else {
            NSLog("Error: Failed to load network set.")
            return
        }

        guard let services = SCNetworkSetCopyServices(networks) as? [SCNetworkService] else {
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
            guard let protoc = SCNetworkServiceCopyProtocol(service, kSCNetworkProtocolTypeProxies) else {
                NSLog("Error: Failed to obtain proxy settings for \(SCNetworkServiceGetName(service)!)")
                continue
            }

            var proxySettings: [String: AnyObject] = [:]
            if let httpProxy = httpProxy {
                proxySettings[kCFNetworkProxiesHTTPProxy as String] = httpProxy.addr as AnyObject
                proxySettings[kCFNetworkProxiesHTTPEnable as String] = true as AnyObject
                proxySettings[kCFNetworkProxiesHTTPSProxy as String] = httpProxy.addr as AnyObject
                proxySettings[kCFNetworkProxiesHTTPSEnable as String] = true as AnyObject
                proxySettings[kCFNetworkProxiesHTTPSPort as String] = httpProxy.port as AnyObject
                proxySettings[kCFNetworkProxiesHTTPPort as String] = httpProxy.port as AnyObject
            } else {
                proxySettings[kCFNetworkProxiesHTTPProxy as String] = "" as AnyObject
                proxySettings[kCFNetworkProxiesHTTPEnable as String] = false as AnyObject
                proxySettings[kCFNetworkProxiesHTTPSProxy as String] = "" as AnyObject
                proxySettings[kCFNetworkProxiesHTTPSEnable as String] = false as AnyObject
                proxySettings[kCFNetworkProxiesHTTPSPort as String] = 0 as AnyObject
                proxySettings[kCFNetworkProxiesHTTPPort as String] = 0 as AnyObject
            }

            if let socksProxy = socksProxy {
                proxySettings[kCFNetworkProxiesSOCKSProxy as String] = socksProxy.addr as AnyObject
                proxySettings[kCFNetworkProxiesSOCKSEnable as String] = true as AnyObject
                proxySettings[kCFNetworkProxiesSOCKSPort as String] = socksProxy.port as AnyObject
            } else {
                proxySettings[kCFNetworkProxiesSOCKSProxy as String] = "" as AnyObject
                proxySettings[kCFNetworkProxiesSOCKSEnable as String] = false as AnyObject
                proxySettings[kCFNetworkProxiesSOCKSPort as String] = 0 as AnyObject
            }

            proxySettings[kCFNetworkProxiesExceptionsList as String] = [
                                                                        "192.168.0.0/16",
                                                                        "10.0.0.0/8",
                                                                        "172.16.0.0/12",
                                                                        "127.0.0.1",
                                                                        "localhost",
                                                                        "*.local"
                                                                        ] as AnyObject

            guard SCNetworkProtocolSetConfiguration(protoc, proxySettings as CFDictionary) else {
                NSLog("Error: Failed to set proxy settings for \(SCNetworkServiceGetName(service)!)")
                continue
            }

            NSLog("Set proxy settings for \(SCNetworkServiceGetName(service)!)")
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

private class Endpoint {
    let addr: String
    let port: UInt16

    init(addr: String, port: UInt16) {
        self.addr = addr
        self.port = port
    }
}
