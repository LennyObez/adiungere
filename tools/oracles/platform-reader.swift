// Probe P01, Apple side: does the platform's own reader hand back each sample exactly as stored, so that
// the digest of its sample buffers equals adiungere-track-fp/1?
//
//     swift tools/oracles/platform-reader.swift RECORDING
//
// Reads every track through the platform reader with no output settings, which is documented to deliver
// samples in their stored form, skips marker buffers that carry no samples, and prints one line per track
// with the sample count, the byte count and the SHA-256 of the samples concatenated in decode order. The
// output is compared with the manifest's track fingerprints by the oracles pipeline.

import AVFoundation
import CryptoKit
import Foundation

struct Line: Encodable {
    let id: Int
    let type: String
    let samples: Int
    let bytes: Int
    let payload_sha256: String
}

func fail(_ message: String) -> Never {
    FileHandle.standardError.write((message + "\n").data(using: .utf8)!)
    exit(1)
}

let arguments = CommandLine.arguments
guard arguments.count == 2 else {
    fail("usage: swift tools/oracles/platform-reader.swift RECORDING")
}

let url = URL(fileURLWithPath: arguments[1])
let asset = AVURLAsset(url: url)
let semaphore = DispatchSemaphore(value: 0)
var lines: [Line] = []
var failure: String?

Task {
    do {
        let tracks = try await asset.load(.tracks)
        for track in tracks {
            let reader = try AVAssetReader(asset: asset)
            let output = AVAssetReaderTrackOutput(track: track, outputSettings: nil)
            output.alwaysCopiesSampleData = false
            guard reader.canAdd(output) else { fail("the reader refuses track \(track.trackID)") }
            reader.add(output)
            guard reader.startReading() else {
                fail("the reader could not start on track \(track.trackID): \(String(describing: reader.error))")
            }

            var hasher = SHA256()
            var samples = 0
            var bytes = 0
            while let buffer = output.copyNextSampleBuffer() {
                // A marker buffer carries no samples: it announces a discontinuity or the end of a segment.
                guard CMSampleBufferGetNumSamples(buffer) > 0, let block = CMSampleBufferGetDataBuffer(buffer) else {
                    continue
                }
                let length = CMBlockBufferGetDataLength(block)
                var data = Data(count: length)
                let status = data.withUnsafeMutableBytes { raw in
                    CMBlockBufferCopyDataBytes(block, atOffset: 0, dataLength: length, destination: raw.baseAddress!)
                }
                guard status == kCMBlockBufferNoErr else { fail("could not copy a sample of track \(track.trackID)") }
                hasher.update(data: data)
                samples += CMSampleBufferGetNumSamples(buffer)
                bytes += length
            }
            if reader.status == .failed {
                fail("the reader failed on track \(track.trackID): \(String(describing: reader.error))")
            }
            let digest = hasher.finalize().map { String(format: "%02x", $0) }.joined()
            let type: String
            switch track.mediaType {
            case .video: type = "video"
            case .audio: type = "audio"
            default: type = track.mediaType.rawValue
            }
            lines.append(Line(id: Int(track.trackID), type: type, samples: samples, bytes: bytes, payload_sha256: digest))
        }
    } catch {
        failure = "\(error)"
    }
    semaphore.signal()
}

semaphore.wait()
if let failure { fail(failure) }

let encoder = JSONEncoder()
encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
let encoded = try encoder.encode(lines)
print(String(decoding: encoded, as: UTF8.self))
