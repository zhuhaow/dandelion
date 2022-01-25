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
        let configFolder = FileManager.default
            .homeDirectoryForCurrentUser
            .appendingPathComponent(".specht2", isDirectory: true)

        do {
            try FileManager.default
                .createDirectory(at: configFolder,
                                 withIntermediateDirectories: true,
                                 attributes: nil)
        } catch {
            Alert.alert(message: "Failed to create config folder: \(error)")
        }

        return configFolder
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
        server.run(name: name, configUrl: configUrl) { result in
            switch result {
            case .success:
                break
            case .failure(let error):
                Alert.alert(message: error)
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

    static func blockShutdown() {
        let sem = DispatchSemaphore(value: 0)
        server.shutdownWith(semaphore: sem)
        sem.wait()
    }

    static func isRunning() -> Bool {
        return server.isRunning()
    }
}

extension Defaults.Keys {
    static let activeConfig = Key<String?>("activeConfig")
}
