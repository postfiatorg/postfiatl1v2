// Downstream PostFiat matched liveness adapter for XRPLF/rippled 3.1.3.
// This is not an upstream XRPLF test.

#include <test/csf/Sim.h>

#include <xrpl/beast/unit_test.h>
#include <xrpl/json/json_reader.h>
#include <xrpl/json/json_writer.h>

#include <algorithm>
#include <chrono>
#include <cstdlib>
#include <filesystem>
#include <fstream>
#include <iterator>
#include <map>
#include <set>
#include <string>
#include <vector>

#include <sys/resource.h>

namespace ripple {
namespace test {

class MatchedLivenessBenchmark_test : public beast::unit_test::suite
{
    using Peer = csf::Peer;
    using PeerGroup = csf::PeerGroup;

    static Json::Value
    readJson(char const* path)
    {
        std::ifstream stream(path);
        if (!stream)
            throw std::runtime_error("cannot open scenario manifest");
        std::string text{
            std::istreambuf_iterator<char>{stream},
            std::istreambuf_iterator<char>{}};
        Json::Value value;
        Json::Reader reader;
        if (!reader.parse(text, value))
            throw std::runtime_error("cannot parse scenario manifest");
        return value;
    }

    static std::vector<std::string>
    strings(Json::Value const& value)
    {
        std::vector<std::string> result;
        for (auto const& row : value)
            result.push_back(row.asString());
        return result;
    }

    static bool
    contains(Json::Value const& value, std::string const& target)
    {
        for (auto const& row : value)
            if (row.asString() == target)
                return true;
        return false;
    }

    static std::uint64_t
    elapsedMicros(std::chrono::steady_clock::time_point start)
    {
        return static_cast<std::uint64_t>(
            duration_cast<std::chrono::microseconds>(
                std::chrono::steady_clock::now() - start)
                .count());
    }

    static std::uint64_t
    processCpuMicros()
    {
        rusage usage{};
        getrusage(RUSAGE_SELF, &usage);
        auto const user = usage.ru_utime.tv_sec * 1'000'000ULL + usage.ru_utime.tv_usec;
        auto const system =
            usage.ru_stime.tv_sec * 1'000'000ULL + usage.ru_stime.tv_usec;
        return user + system;
    }

    static std::uint64_t
    peakRssKib()
    {
        rusage usage{};
        getrusage(RUSAGE_SELF, &usage);
        return static_cast<std::uint64_t>(usage.ru_maxrss);
    }

    static std::uint64_t
    openDescriptors()
    {
        std::error_code error;
        std::filesystem::directory_iterator descriptors{"/proc/self/fd", error};
        if (error)
            return 0;
        return static_cast<std::uint64_t>(std::distance(
            begin(descriptors), std::filesystem::directory_iterator{}));
    }

    static std::chrono::milliseconds
    linkDelay(Json::Value const& scenario, std::size_t left, std::size_t right)
    {
        auto const base = scenario["faults"]["latency_ms"]["base"].asUInt();
        auto const jitter = scenario["faults"]["latency_ms"]["jitter"].asUInt();
        auto const seed = scenario["seed"].asUInt();
        auto const offset = jitter == 0
            ? 0
            : (seed + left * 7919 + right * 104729) % (jitter + 1);
        return std::chrono::milliseconds{base + offset};
    }

    static PeerGroup
    groupFrom(std::vector<Peer*> const& peers, std::set<std::size_t> const& indices)
    {
        std::vector<Peer*> selected;
        for (auto index : indices)
            selected.push_back(peers[index]);
        return PeerGroup{selected};
    }

    static void
    connectPair(
        Json::Value const& scenario,
        std::vector<Peer*> const& peers,
        std::size_t left,
        std::size_t right)
    {
        if (left != right)
            peers[left]->connect(*peers[right], linkDelay(scenario, left, right));
    }

    static void
    connectAll(
        Json::Value const& scenario,
        std::vector<Peer*> const& peers,
        std::set<std::size_t> const& unavailable)
    {
        for (std::size_t left = 0; left < peers.size(); ++left)
            for (std::size_t right = left + 1; right < peers.size(); ++right)
                if (!unavailable.count(left) && !unavailable.count(right))
                    connectPair(scenario, peers, left, right);
    }

    static void
    connectPartitions(
        Json::Value const& scenario,
        std::vector<Peer*> const& peers,
        std::map<std::string, std::size_t> const& index,
        std::set<std::size_t> const& unavailable)
    {
        for (auto const& partition : scenario["faults"]["partitions"])
        {
            auto members = strings(partition);
            for (std::size_t left = 0; left < members.size(); ++left)
                for (std::size_t right = left + 1; right < members.size(); ++right)
                {
                    auto const a = index.at(members[left]);
                    auto const b = index.at(members[right]);
                    if (!unavailable.count(a) && !unavailable.count(b))
                        connectPair(scenario, peers, a, b);
                }
        }
    }

    static Json::Value
    runScenario(Json::Value const& scenario)
    {
        using namespace std::chrono;
        auto const wallStart = steady_clock::now();
        auto const cpuStart = processCpuMicros();
        csf::Sim sim;
        auto const validators = strings(scenario["validators"]);
        auto all = sim.createGroup(validators.size());
        std::vector<Peer*> peers{all.begin(), all.end()};
        std::map<std::string, std::size_t> index;
        for (std::size_t i = 0; i < validators.size(); ++i)
            index.emplace(validators[i], i);

        for (std::size_t i = 0; i < validators.size(); ++i)
        {
            auto const& validator = validators[i];
            for (auto const& trusted : scenario["local_unls"][validator])
                peers[i]->trust(*peers[index.at(trusted.asString())]);
            peers[i]->quorum = scenario["local_quorums"][validator].asUInt();
        }

        std::set<std::size_t> unavailable;
        for (auto const* category : {"offline", "censored"})
            for (auto const& validator : scenario["faults"][category])
                unavailable.insert(index.at(validator.asString()));
        for (auto const& validator : scenario["faults"]["actively_byzantine"])
        {
            auto const i = index.at(validator.asString());
            if (contains(scenario["faults"]["censored"], validator.asString()))
                peers[i]->runAsValidator = false;
        }
        for (auto i : unavailable)
            peers[i]->runAsValidator = false;

        std::set<std::size_t> correct;
        for (std::size_t i = 0; i < peers.size(); ++i)
            if (!unavailable.count(i) &&
                !contains(scenario["faults"]["actively_byzantine"], validators[i]))
                correct.insert(i);
        auto correctGroup = groupFrom(peers, correct);
        auto correctProgressed = [&]() {
            return std::all_of(correct.begin(), correct.end(), [&](auto i) {
                return peers[i]->fullyValidatedLedger.seq() >
                    csf::Ledger::Seq{0};
            });
        };

        sim.net.configureFaults(
            scenario["faults"]["packet_loss_every"].asUInt(),
            scenario["faults"]["duplicate_every"].asUInt(),
            scenario["faults"]["reorder_every"].asUInt(),
            milliseconds{scenario["faults"]["reorder_extra_ms"].asUInt()});

        bool const partitioned = scenario["faults"]["partitions"].size() != 0;
        if (partitioned)
            connectPartitions(scenario, peers, index, unavailable);
        else
            connectAll(scenario, peers, unavailable);

        for (std::size_t i = 0; i < peers.size(); ++i)
            if (!unavailable.count(i))
                peers[i]->submit(csf::Tx{static_cast<std::uint32_t>(i + 1)});

        auto runUntilSynchronized = [&](milliseconds remaining) {
            auto const deadline = sim.scheduler.now() + remaining;
            while (sim.scheduler.now() < deadline && !correctProgressed())
            {
                if (!sim.scheduler.step_one())
                    break;
            }
        };

        Json::Value pre{Json::objectValue};
        if (partitioned)
        {
            auto const healAt = scenario["faults"]["heal_at_ms"].asUInt();
            sim.run(milliseconds{healAt});
            pre["branches"] = static_cast<Json::Value::UInt>(sim.branches());
            pre["synchronized"] = sim.synchronized();
            pre["virtual_elapsed_ms"] = static_cast<Json::Value::UInt>(
                duration_cast<milliseconds>(sim.scheduler.now().time_since_epoch())
                    .count());
            connectAll(scenario, peers, unavailable);
            sim.run(seconds{1});
            runUntilSynchronized(
                milliseconds{scenario["timeout_ms"].asUInt() - healAt - 1000});
        }
        else if (scenario["transition"]["kind"].asString() == "key_rotation")
        {
            sim.run(milliseconds{scenario["timeout_ms"].asUInt() / 2});
            for (auto const& validator : scenario["transition"]["rotated"])
                ++peers[index.at(validator.asString())]->key.second;
            sim.run(seconds{1});
            runUntilSynchronized(
                milliseconds{scenario["timeout_ms"].asUInt() / 2 - 1000});
        }
        else
        {
            sim.run(seconds{1});
            runUntilSynchronized(
                milliseconds{scenario["timeout_ms"].asUInt() - 1000});
        }

        bool decided = !correct.empty();
        Json::Value nodes{Json::arrayValue};
        for (auto i : correct)
        {
            auto const validated = static_cast<std::uint32_t>(
                peers[i]->fullyValidatedLedger.seq());
            decided = decided && validated > 0;
            Json::Value row{Json::objectValue};
            row["validator"] = validators[i];
            row["completed_ledgers"] = peers[i]->completedLedgers;
            row["fully_validated_sequence"] = validated;
            row["last_closed_sequence"] = static_cast<std::uint32_t>(
                peers[i]->lastClosedLedger.seq());
            row["local_quorum"] = static_cast<Json::Value::UInt>(peers[i]->quorum);
            nodes.append(row);
        }
        auto const branches = correct.empty() ? 0 : sim.branches(correctGroup);
        auto const conflicts = branches > 0 ? branches - 1 : 0;
        bool const synchronized = correct.empty() || sim.synchronized(correctGroup);
        auto const expected = scenario["expected"]["post_heal"].asString();
        auto const modelScope = scenario["expected"]["model_scope"].asString();
        bool expectationPassed = conflicts ==
            scenario["expected"]["conflicting_decisions"].asUInt();
        if (expected == "one_decision")
            expectationPassed = expectationPassed && decided && synchronized;
        else if (expected == "safe_halt")
            expectationPassed = expectationPassed && !decided;
        else if (expected == "one_decision_or_safe_halt")
            expectationPassed = expectationPassed && conflicts == 0;
        else if (modelScope != "characterize")
            expectationPassed = false;

        auto const stats = sim.net.faultStats();
        Json::Value result{Json::objectValue};
        result["schema"] = "postfiat-rippled-matched-case-v1";
        result["case_id"] = scenario["id"];
        result["topology_id"] = scenario["topology_id"];
        result["fault_class"] = scenario["fault_class"];
        result["model_scope"] = modelScope;
        result["validator_count"] = static_cast<Json::Value::UInt>(validators.size());
        result["decided"] = decided;
        result["safe_halt"] = !decided;
        result["synchronized"] = synchronized;
        result["branches"] = static_cast<Json::Value::UInt>(branches);
        result["conflicting_decisions"] = static_cast<Json::Value::UInt>(conflicts);
        result["expectation_passed"] = expectationPassed || modelScope == "characterize";
        result["nodes"] = nodes;
        result["pre_heal"] = pre;
        result["local_quorums"] = scenario["local_quorums"];
        result["view_detail"] = scenario["view_detail"];
        result["transition"] = scenario["transition"];
        result["network_faults"]["sent"] = static_cast<Json::Value::UInt>(stats.sent);
        result["network_faults"]["delivered"] = static_cast<Json::Value::UInt>(stats.delivered);
        result["network_faults"]["dropped"] = static_cast<Json::Value::UInt>(stats.dropped);
        result["network_faults"]["duplicated"] = static_cast<Json::Value::UInt>(stats.duplicated);
        result["network_faults"]["reordered"] = static_cast<Json::Value::UInt>(stats.reordered);
        result["equivocation_mapping"] =
            scenario["fault_class"].asString() == "equivocation"
            ? "native CSF omission control; signed-equivocation lock is measured by the Cobalt adapter"
            : "not_applicable";
        auto const convergenceVirtualMs = static_cast<Json::Value::UInt>(
            duration_cast<milliseconds>(sim.scheduler.now().time_since_epoch())
                .count());
        auto const converged = correctProgressed() && sim.synchronized(correctGroup);
        result["convergence_virtual_ms"] =
            converged ? Json::Value{convergenceVirtualMs} : Json::Value{};
        result["timeout_or_quiescence_virtual_ms"] = convergenceVirtualMs;
        auto recoveryStartMs = Json::Value::UInt{0};
        if (scenario["faults"]["partitions"].size() != 0)
            recoveryStartMs = scenario["faults"]["heal_at_ms"].asUInt();
        else if (scenario["transition"]["kind"].asString() == "key_rotation")
            recoveryStartMs = scenario["timeout_ms"].asUInt() / 2;
        result["recovery_virtual_ms"] =
            converged
            ? Json::Value{static_cast<Json::Value::UInt>(
                  convergenceVirtualMs - recoveryStartMs)}
            : Json::Value{};
        result["resource_accounting"] = Json::Value{Json::objectValue};
        result["resource_accounting"]["actual_wall_micros"] =
            static_cast<Json::Value::UInt>(elapsedMicros(wallStart));
        result["resource_accounting"]["process_cpu_micros"] =
            static_cast<Json::Value::UInt>(
                processCpuMicros() - cpuStart);
        result["resource_accounting"]["process_peak_rss_kib"] =
            static_cast<Json::Value::UInt>(peakRssKib());
        result["resource_accounting"]["process_open_descriptors"] =
            static_cast<Json::Value::UInt>(openDescriptors());
        result["resource_accounting"]["disk_delta_bytes"] = 0;
        result["resource_accounting"]["serialized_wire_bytes"] = 0;
        result["resource_accounting"]["transport_model"] = "in-memory CSF callback";
        return result;
    }

public:
    void
    run() override
    {
        auto const* manifestPath =
            std::getenv("POSTFIAT_MATCHED_SCENARIO_MANIFEST");
        auto const* outputPath =
            std::getenv("POSTFIAT_RIPPLED_BENCHMARK_OUTPUT");
        if (!BEAST_EXPECT(manifestPath != nullptr && outputPath != nullptr))
            return;
        auto const manifest = readJson(manifestPath);
        if (!BEAST_EXPECT(
                manifest["schema"].asString() ==
                "postfiat-cobalt-rippled-scenario-manifest-v1"))
            return;
        testcase("matched scenario manifest");
        Json::Value results{Json::arrayValue};
        std::size_t passed = 0;
        std::size_t conflicts = 0;
        for (auto const& scenario : manifest["cases"])
        {
            auto result = runScenario(scenario);
            if (result["expectation_passed"].asBool())
                ++passed;
            conflicts += result["conflicting_decisions"].asUInt();
            std::cout << "RIPPLED_MATCHED_CASE "
                      << result["case_id"].asString() << " decided="
                      << result["decided"].asBool() << " branches="
                      << result["branches"].asUInt() << " pass="
                      << result["expectation_passed"].asBool() << std::endl;
            results.append(std::move(result));
        }
        Json::Value report{Json::objectValue};
        report["schema"] = "postfiat-rippled-matched-benchmark-report-v1";
        report["rippled_commit"] =
            "46b241ace8b30d9c9775d60ffba7d24b21903896";
        report["native_control"] =
            "upstream src/test/csf with Consensus_test::testFork baseline";
        report["downstream_adapter"] = true;
        report["scenario_manifest_sha256"] = manifest["manifest_sha256"];
        report["case_count"] = static_cast<Json::Value::UInt>(results.size());
        report["passed_case_count"] = static_cast<Json::Value::UInt>(passed);
        report["conflicting_decision_count"] =
            static_cast<Json::Value::UInt>(conflicts);
        report["calculate_quorum"] =
            "max(ceil(0.8 * effectiveUNL), ceil(0.6 * localUNL)); local only";
        report["results"] = std::move(results);
        report["status"] = passed == manifest["cases"].size()
            ? "passed"
            : "failed";
        std::ofstream output(outputPath);
        output << Json::StyledWriter{}.write(report);
        output.close();
        std::cout << "RIPPLED_MATCHED_BENCHMARK cases="
                  << manifest["cases"].size() << " passed=" << passed
                  << " conflicts=" << conflicts << " status="
                  << report["status"].asString() << std::endl;
        BEAST_EXPECT(passed == manifest["cases"].size());
    }
};

BEAST_DEFINE_TESTSUITE(MatchedLivenessBenchmark, consensus, ripple);

}  // namespace test
}  // namespace ripple
