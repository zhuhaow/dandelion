//
//  ConfigManager.swift
//  Specht2
//
//  Created by Zhuhao Wang on 2021/12/15.
//

import AppKit
import Defaults

class ConfigManager {
    static var configPath: URL {
        FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).first!
    }

    static var configs: [String: URL] = [:]

    static var activeConfig: String? {
        Defaults[.activeConfig]
    }

    static func openConfigFolder() {
        NSWorkspace.shared.open(configPath)
    }

    static func reloadConfigs() {
        do {
            let paths = try FileManager.default.contentsOfDirectory(at: configPath,
                                                                    includingPropertiesForKeys: nil)
                .filter {
                $0.pathExtension == "ron"
            }

            configs = Dictionary(uniqueKeysWithValues: paths.map {
                ($0.deletingPathExtension().lastPathComponent, $0)
            })
        } catch {
            Alert.alert(message: "Failed to load config files due to error: \(error)")
        }
    }

    static func run(name: String) {
        fatalError("not implemented")
    }

    static func stop() {
        fatalError("not implemented")
    }

    static func initialize() {
        reloadConfigs()
    }
}
