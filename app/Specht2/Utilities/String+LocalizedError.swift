//
//  String+LocalizedError.swift
//  Specht2
//
//  Created by Zhuhao Wang on 2021/12/26.
//

import Foundation

extension String: LocalizedError {
    public var errorDescription: String? { return self }
}
