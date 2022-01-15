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
