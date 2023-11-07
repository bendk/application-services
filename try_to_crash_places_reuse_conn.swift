import Foundation
import places_mod

let databasePath = CommandLine.arguments[1]
let iteration = CommandLine.arguments[2]
print("trying to crash places [reuse-connection] (\(iteration))")

let api = try! placesApiNew(dbPath: databasePath)
let reader = try! api.newConnection(connType: ConnectionType.readOnly)
let writer = try! api.newConnection(connType: ConnectionType.readWrite)
for i in 0...100 {
    // Race a thread that calls `bookmarksGetTree` and one that runs maintenance
    Task {
        let RootGUID = "root________"
        let result = try! reader.bookmarksGetTree(itemGuid: RootGUID)
    }

    Task {
        let _ = try writer.runMaintenancePrune(dbSizeLimit: 1_000_000)
        try writer.runMaintenanceVacuum()
        try writer.runMaintenanceOptimize()
        try writer.runMaintenanceCheckpoint()
        try writer.runMaintenanceGenerateItems()
    }
}
