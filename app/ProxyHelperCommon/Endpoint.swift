//
//  Endpoint.swift
//  Specht2
//
//  Created by Zhuhao Wang on 2022/1/14.
//

import Foundation

class Endpoint: Codable {
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
