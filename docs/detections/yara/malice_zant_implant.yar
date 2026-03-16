import "pe"

rule MAL_Malice_Zant_Implant_Win
{
    meta:
        author = "OpenAI Codex"
        date = "2026-03-15"
        description = "Detects the current Zant implant runtime bundled in the Malice repository"
        family = "Malice"
        implant = "Zant"
        platform = "Windows"
        reference_1 = "implant/zant/runtime/runtime.cpp"
        reference_2 = "implant/zant/runtime/transport.cpp"
        reference_3 = "implant/zant/runtime/config.cpp"

    strings:
        $s1 = "X-Malice-Register: coff-loader-v1" ascii
        $s2 = "zant-coff-loader/1.0" ascii
        $s3 = "Content-Type: application/octet-stream" ascii
        $s4 = "\\zant\\clientid.txt" ascii
        $s5 = "[zant] registration succeeded" ascii
        $s6 = "registration rejected for persisted clientid; retrying as a new implant" ascii
        $s7 = "\"implant_type\":\"coff_loader\"" ascii
        $s8 = "\"result_encoding\":\"utf8\"" ascii
        $s9 = "execute_coff" ascii
        $s10 = "--heartbeat-seconds" ascii
        $s11 = "--poll-seconds" ascii
        $s12 = "--clientid-path" ascii
        $s13 = "--once" ascii

    condition:
        uint16(0) == 0x5A4D and
        pe.is_pe and
        6 of ($s1,$s2,$s3,$s4,$s5,$s6,$s7,$s8,$s9) and
        2 of ($s10,$s11,$s12,$s13)
}
