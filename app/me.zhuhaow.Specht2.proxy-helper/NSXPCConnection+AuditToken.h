//
//  NSXPCConnection+AuditToken.h
//  Specht2
//
//  Created by Zhuhao Wang on 2021/12/24.
//
// From https://github.com/aronskaya/smjobbless/blob/master/com.smjobblesssample.installer/NSXPCConnection%2BAuditToken.h

#ifndef NSXPCConnection_AuditToken_h
#define NSXPCConnection_AuditToken_h

@import Foundation;

@interface NSXPCConnection (AuditToken)

// Apple uses this property internally to verify XPC connections.
// There is no safe pulicly available alternative (check by client pid, for example, is racy)
@property (nonatomic, readonly) audit_token_t auditToken;

@end

#endif /* NSXPCConnection_AuditToken_h */
