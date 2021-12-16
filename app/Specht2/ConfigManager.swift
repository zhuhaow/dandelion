//
//  ConfigManager.swift
//  Specht2
//
//  Created by Zhuhao Wang on 2021/12/15.
//

import AppKit
import Defaults

class ConfigManager {
    static var server = Server()

    static var configPath: URL {
        FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).first!
    }

    static var configs: [String: URL] = [:]

    static var activeConfig: String? {
        return Defaults[.activeConfig]
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

            if let name = Defaults[.activeConfig], configs.keys.contains(name) {
                run(name: name)
            } else {
                stop()
                Defaults[.activeConfig] = nil
            }
        } catch {
            Alert.alert(message: "Failed to load config files due to error: \(error)")
        }
    }

    static func run(name: String) {
        guard let configUrl = configs[name] else {
            reloadConfigs()
            return
        }

        Defaults[.activeConfig] = name
        server.run(name: name, configUrl: configUrl) { err in
            if let err = err {
                Alert.alert(message: err)
            }
        }
    }

    static func stop() {
        Defaults[.activeConfig] = nil
        server.shutdown()
    }

    static func initialize() {
        reloadConfigs()
    }

    static func isRunning() -> Bool {
        return server.isRunning()
    }
}
