//
//  Log.swift
//  Specht2
//
//  Created by Zhuhao Wang on 2022/1/27.
//

import Foundation
import Defaults

enum LogLevel: Codable, Defaults.Serializable, CaseIterable {
    case off
    case error
    case warn
    case info
    case debug
    case trace
    
    func toFfiLogLevel() -> UInt {
        switch self {
        case .off:
            return UInt(LEVEL_FILTER_OFF.rawValue)
        case .error:
            return UInt(LEVEL_FILTER_ERROR.rawValue)
        case .warn:
            return UInt(LEVEL_FILTER_WARN.rawValue)
        case .info:
            return UInt(LEVEL_FILTER_INFO.rawValue)
        case .debug:
            return UInt(LEVEL_FILTER_DEBUG.rawValue)
        case .trace:
            return UInt(LEVEL_FILTER_TRACE.rawValue)
        }
    }
}

func setLogLevel(level: LogLevel) {
    Defaults[.logLevel] = level
    specht2_set_log_level(level.toFfiLogLevel())
}

extension Defaults.Keys {
    static let logLevel = Key<LogLevel>("logLevel", default: LogLevel.warn)
}

class Log {
    static func initialize() {
        setLogLevel(level: Defaults[.logLevel])
    }
}
