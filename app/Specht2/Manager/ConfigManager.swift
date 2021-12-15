//
//  ConfigManager.swift
//  Specht2
//
//  Created by Zhuhao Wang on 2021/12/15.
//

import AppKit

class ConfigManager {
    static var configPath: URL {
        get {
            FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).first!
        }
    }

    static func openConfigFolder() {
        NSWorkspace.shared.open(configPath)
    }
}
