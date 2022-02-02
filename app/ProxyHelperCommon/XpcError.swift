//
//  XpcError.swift
//  Specht2
//
//  Created by Zhuhao Wang on 2022/1/16.
//

import Foundation

func takeFfiLastError() -> XpcError? {
    let len = specht2_get_last_error_len()
    guard len > 0 else {
        return nil
    }

    var buf = Data(count: Int(len))

    return buf.withUnsafeMutableBytes {
        let ptr = $0.bindMemory(to: CChar.self)
        let result = specht2_take_last_error($0.bindMemory(to: CChar.self).baseAddress!, UInt(len))

        guard result > 0 else {
            return nil
        }

        return XpcError(String(cString: ptr.baseAddress!))
    }
}

struct XpcError: LocalizedError, Codable {
    let message: String

    init(_ message: String) {
        self.message = message
    }

    public var errorDescription: String? {
        return message
    }
}
