using LiteDB;

namespace OmniSpot.Configuration.Migrations;

internal interface IMigration
{
    int Version { get; }
    void Up(LiteDatabase db);
}