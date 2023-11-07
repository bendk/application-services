import Foundation
import places_mod

let databasePath = CommandLine.arguments[1]
print("opening \(databasePath)")
let api = try! placesApiNew(dbPath: databasePath)

// Race a thread that calls `bookmarksGetTree` and one that runs maintenance
Task {
    let RootGUID = "root________"
    let reader = try! api.newConnection(connType: ConnectionType.readOnly)
    let result = try! reader.bookmarksGetTree(itemGuid: RootGUID)
}

Task {
    let writer = try! api.newConnection(connType: ConnectionType.readWrite)
    let _ = try writer.runMaintenancePrune(dbSizeLimit: 1_000_000)
    try writer.runMaintenanceVacuum()
    try writer.runMaintenanceOptimize()
    try writer.runMaintenanceCheckpoint()
    try writer.runMaintenanceGenerateItems()
}

print("done")
