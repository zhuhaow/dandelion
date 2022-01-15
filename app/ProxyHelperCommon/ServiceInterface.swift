//
//  ServiceInterface.swift
//  Specht2
//
//  Created by Zhuhao Wang on 2021/12/23.
//

import Foundation
import System

protocol ServiceInterface {
    func setSocks5Proxy(endpoint: Endpoint?) async throws
    func setHttpProxy(endpoint: Endpoint?) async throws
    func setDns(endpoint: Endpoint?) async throws

    func createTunInterface(subnet: String) async throws -> FileDescriptor

    func currentVersion() async throws -> String
}

class ServiceInterfaceMethodName {
    static let setSocks5Proxy = "me.zhuhaow.Specht2.setSocks5Proxy"
    static let setHttpProxy = "me.zhuhaow.Specht2.setHttpProxy"
    static let setDns = "me.zhuhaow.Specht2.setDns"
    static let createTunInterface = "me.zhuhaow.Specht2.createTunInterface"
    static let currentVersion = "me.zhuhaow.Specht2.currentVersion"
}
