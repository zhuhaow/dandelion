//
//  ProxyHelperInterface.swift
//  Specht2
//
//  Created by Zhuhao Wang on 2021/12/23.
//

import Foundation

@objc protocol ProxyHelperInterface {
    func setProxy(setSocks5: Bool, socks5Address: String, socks5Port: UInt16,
                  setHttp: Bool, httpAddress: String, httpPort: UInt16)

    func currentVersion(completionHandler: @escaping (String) -> Void)
}
