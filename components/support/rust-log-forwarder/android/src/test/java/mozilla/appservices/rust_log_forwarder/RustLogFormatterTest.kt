/* Any copyright is dedicated to the Public Domain.
   http://creativecommons.org/publicdomain/zero/1.0/ */

package mozilla.appservices.rust_log_forwarder

import org.junit.Test

class RustLogForwarderPerformanceTest {
    @Test
    fun testLoggingPerformance() {
        setLogger(NullLogger());
        testPerformance()
    }
}

class NullLogger() : Logger {
    override fun log(record: Record) {
    }
}
